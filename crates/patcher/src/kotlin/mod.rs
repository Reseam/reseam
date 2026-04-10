pub(crate) mod bytecode;
pub(crate) mod convert;
mod log_host;
mod manifest;
mod options;
mod files;
mod resources;
pub mod types;
mod xml;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use boltffi::export;
use jni::objects::{JObject, JObjectArray, JValue};
use jni::{InitArgsBuilder, JNIVersion, JavaVM};
use stitch_apk::stitch_dex::{DexFile, EncodedMethod};
use stitch_apk::AxmlDocument;
use tracing::warn;

use crate::context::PatchContext;
use crate::error::{PatcherError, Result};
use crate::options::OptionDeclaration;
use crate::patch::{Compatibility, Patch};

#[derive(Clone, Copy)]
pub(crate) struct MethodHandle {
    pub dex_idx: usize,
    pub class_idx: usize,
    pub method_idx: usize,
    pub is_virtual: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct ClassHandle {
    pub dex_idx: usize,
    pub class_idx: usize,
}

#[derive(Default)]
pub(crate) struct HandleTable {
    methods: Vec<MethodHandle>,
    classes: Vec<ClassHandle>,
    method_lookup: HashMap<(usize, usize, usize, bool), u32>,
    class_lookup: HashMap<(usize, usize), u32>,
}

impl HandleTable {
    pub fn alloc_method(
        &mut self,
        dex_idx: usize,
        class_idx: usize,
        method_idx: usize,
        is_virtual: bool,
    ) -> u32 {
        let key = (dex_idx, class_idx, method_idx, is_virtual);
        if let Some(&h) = self.method_lookup.get(&key) {
            return h;
        }
        let h = self.methods.len() as u32;
        self.methods.push(MethodHandle {
            dex_idx,
            class_idx,
            method_idx,
            is_virtual,
        });
        self.method_lookup.insert(key, h);
        h
    }

    pub fn get_method(&self, handle: u32) -> Option<MethodHandle> {
        self.methods.get(handle as usize).copied()
    }

    pub fn alloc_class(&mut self, dex_idx: usize, class_idx: usize) -> u32 {
        let key = (dex_idx, class_idx);
        if let Some(&h) = self.class_lookup.get(&key) {
            return h;
        }
        let h = self.classes.len() as u32;
        self.classes.push(ClassHandle { dex_idx, class_idx });
        self.class_lookup.insert(key, h);
        h
    }

    pub fn get_class(&self, handle: u32) -> Option<ClassHandle> {
        self.classes.get(handle as usize).copied()
    }
}

thread_local! {
    static CTX_PTR: Cell<*mut ()> = const { Cell::new(std::ptr::null_mut()) };
    pub(crate) static HANDLES: RefCell<HandleTable> = RefCell::new(HandleTable::default());
    pub(crate) static XML_DOCUMENTS: RefCell<Vec<Option<(AxmlDocument, String)>>> = RefCell::new(Vec::new());
    pub(crate) static PENDING_ELEMENTS: RefCell<Vec<xml::PendingElement>> = RefCell::new(Vec::new());
    pub(crate) static BUNDLE_DIR: RefCell<Option<PathBuf>> = RefCell::new(None);
}

struct CtxGuard;

impl CtxGuard {
    fn enter(ctx: &mut PatchContext<'_>) -> Self {
        CTX_PTR.with(|cell| {
            assert!(
                cell.get().is_null(),
                "nested CtxGuard: previous context was not cleaned up"
            );
            cell.set(ctx as *mut PatchContext as *mut ());
        });
        HANDLES.with(|h| *h.borrow_mut() = HandleTable::default());
        CtxGuard
    }
}

impl Drop for CtxGuard {
    fn drop(&mut self) {
        CTX_PTR.with(|cell| cell.set(std::ptr::null_mut()));
        HANDLES.with(|h| *h.borrow_mut() = HandleTable::default());
        XML_DOCUMENTS.with(|docs| docs.borrow_mut().clear());
        PENDING_ELEMENTS.with(|pe| pe.borrow_mut().clear());
    }
}

include!(concat!(env!("OUT_DIR"), "/jni_natives.rs"));

fn register_jni_natives(env: &mut jni::JNIEnv<'_>, loader: &JObject<'_>) -> Result<()> {
    let name = env
        .new_string("dev.stitch.patch.Native")
        .map_err(|e| jvm_err(format!("new_string: {e}")))?;
    let native_class = env
        .call_method(
            loader,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[JValue::Object(&name)],
        )
        .and_then(|v| v.l())
        .map_err(|e| jvm_err(format!("load Native class: {e}")))?;
    let class_ref: jni::objects::JClass = native_class.into();
    env.register_native_methods(class_ref, &jni_native_methods())
        .map_err(|e| jvm_err(format!("register natives: {e}")))?;
    Ok(())
}

pub(crate) fn with_ctx<R>(f: impl FnOnce(&mut PatchContext<'_>) -> R) -> R {
    CTX_PTR.with(|cell| {
        let ptr = cell.get();
        assert!(!ptr.is_null(), "patch context is not active");
        let ctx = unsafe { &mut *(ptr as *mut PatchContext<'_>) };
        f(ctx)
    })
}

#[export]
pub fn ctx_is_active() -> bool {
    CTX_PTR.with(|cell| !cell.get().is_null())
}

pub(crate) fn with_handles<R>(f: impl FnOnce(&mut HandleTable) -> R) -> R {
    HANDLES.with(|cell| f(&mut cell.borrow_mut()))
}

pub(crate) fn try_get_method_ref(dex: &DexFile, mh: MethodHandle) -> Option<&EncodedMethod> {
    let class = dex.classes.get(mh.class_idx)?;
    let data = class.class_data.as_ref()?;
    if mh.is_virtual {
        data.virtual_methods.get(mh.method_idx)
    } else {
        data.direct_methods.get(mh.method_idx)
    }
}

pub(crate) fn try_get_method_mut(
    dex: &mut DexFile,
    mh: MethodHandle,
) -> Option<&mut EncodedMethod> {
    let class = dex.classes.get_mut(mh.class_idx)?;
    let data = class.class_data.as_mut()?;
    if mh.is_virtual {
        data.virtual_methods.get_mut(mh.method_idx)
    } else {
        data.direct_methods.get_mut(mh.method_idx)
    }
}

pub(crate) fn get_method_ref(dex: &DexFile, mh: MethodHandle) -> Option<&EncodedMethod> {
    let result = try_get_method_ref(dex, mh);
    if result.is_none() {
        warn!(
            dex_idx = mh.dex_idx,
            class_idx = mh.class_idx,
            method_idx = mh.method_idx,
            is_virtual = mh.is_virtual,
            "invalid method handle"
        );
    }
    result
}

pub(crate) fn get_method_mut(dex: &mut DexFile, mh: MethodHandle) -> Option<&mut EncodedMethod> {
    let result = try_get_method_mut(dex, mh);
    if result.is_none() {
        warn!(
            dex_idx = mh.dex_idx,
            class_idx = mh.class_idx,
            method_idx = mh.method_idx,
            is_virtual = mh.is_virtual,
            "invalid mutable method handle"
        );
    }
    result
}

pub(crate) fn find_method_location(
    ctx: &PatchContext<'_>,
    dex_idx: usize,
    method: &EncodedMethod,
) -> Option<(usize, usize, bool)> {
    let dex = ctx.dex_file(dex_idx)?;
    for (ci, class) in dex.classes.iter().enumerate() {
        if let Some(data) = &class.class_data {
            for (mi, m) in data.direct_methods.iter().enumerate() {
                if std::ptr::eq(m, method) {
                    return Some((ci, mi, false));
                }
            }
            for (mi, m) in data.virtual_methods.iter().enumerate() {
                if std::ptr::eq(m, method) {
                    return Some((ci, mi, true));
                }
            }
        }
    }
    None
}

pub(crate) fn method_match_location(
    ctx: &PatchContext<'_>,
    dex_idx: usize,
    mm: &stitch_apk::stitch_dex::MethodMatch<'_>,
) -> Option<(usize, usize, bool)> {
    find_method_location(ctx, dex_idx, mm.method)
}

pub(crate) fn scan_location(
    ctx: &PatchContext<'_>,
    dex_idx: usize,
    class_idx: usize,
    method_idx: usize,
) -> Option<(usize, bool)> {
    let dex = ctx.dex_file(dex_idx)?;
    let class = dex.classes.get(class_idx)?;
    let data = class.class_data.as_ref()?;
    let is_virtual = method_idx >= data.direct_methods.len();
    let actual = if is_virtual {
        method_idx - data.direct_methods.len()
    } else {
        method_idx
    };
    Some((actual, is_virtual))
}

fn jvm_err(msg: impl std::fmt::Display) -> PatcherError {
    PatcherError::Jvm {
        reason: msg.to_string(),
    }
}

static JVM: OnceLock<std::result::Result<JavaVM, String>> = OnceLock::new();

fn find_java_home() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("JAVA_HOME") {
        let path = PathBuf::from(home);
        if path.is_dir() {
            return Some(path);
        }
    }
    let output = std::process::Command::new("java")
        .arg("-XshowSettings:property")
        .arg("-version")
        .output()
        .ok()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    for line in stderr.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("java.home") {
            if let Some(val) = trimmed.split('=').nth(1) {
                let path = PathBuf::from(val.trim());
                if path.is_dir() {
                    return Some(path);
                }
            }
        }
    }
    None
}

fn find_jvm_lib(java_home: &Path) -> Option<PathBuf> {
    let candidates = [
        "lib/server/libjvm.so",
        "lib/amd64/server/libjvm.so",
        "lib/client/libjvm.so",
        "jre/lib/server/libjvm.so",
        "jre/lib/amd64/server/libjvm.so",
        "lib/server/libjvm.dylib",
        "lib/libjvm.dylib",
    ];
    for candidate in &candidates {
        let path = java_home.join(candidate);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

fn get_or_init_jvm() -> Result<&'static JavaVM> {
    let result = JVM.get_or_init(|| {
        let init = || -> std::result::Result<JavaVM, String> {
            let java_home = find_java_home()
                .ok_or_else(|| "JAVA_HOME not set and java not found on PATH".to_string())?;
            let jvm_lib = find_jvm_lib(&java_home)
                .ok_or_else(|| format!("libjvm not found in {}", java_home.display()))?;

            unsafe {
                setup_jvm_library_path(&jvm_lib).map_err(|e| format!("{e}"))?;
            }

            let lib_path = detect_runtime_library_dir()
                .unwrap_or_else(|| PathBuf::from("."));
            let jvm_args = InitArgsBuilder::new()
                .version(JNIVersion::V8)
                .option(format!(
                    "-Xmx{}",
                    std::env::var("STITCH_JVM_HEAP").unwrap_or_else(|_| "256m".into())
                ))
                .option(format!("-Djava.library.path={}", lib_path.display()))
                .build()
                .map_err(|e| format!("JVM args: {e}"))?;
            JavaVM::new(jvm_args).map_err(|e| format!("JVM init: {e}"))
        };
        init()
    });
    match result {
        Ok(jvm) => Ok(jvm),
        Err(msg) => Err(jvm_err(msg)),
    }
}

fn detect_runtime_library_dir() -> Option<PathBuf> {
    let candidates = std::env::current_exe().ok().map(|exe| {
        let mut dirs = Vec::new();
        if let Some(dir) = exe.parent() {
            dirs.push(dir.to_path_buf());
            if let Some(parent) = dir.parent() {
                dirs.push(parent.to_path_buf());
            }
        }
        dirs
    })?;

    for dir in candidates {
        let so = dir.join("libstitch_patcher_jni.so");
        let dylib = dir.join("libstitch_patcher_jni.dylib");
        let dll = dir.join("stitch_patcher_jni.dll");
        if so.exists() || dylib.exists() || dll.exists() {
            return Some(dir);
        }
    }

    None
}

unsafe fn setup_jvm_library_path(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| jvm_err("no parent dir for libjvm"))?;
    let parent_str = parent.to_str().unwrap_or("");
    if let Ok(current) = std::env::var("LD_LIBRARY_PATH") {
        if !current.contains(parent_str) {
            std::env::set_var("LD_LIBRARY_PATH", format!("{parent_str}:{current}"));
        }
    } else {
        std::env::set_var("LD_LIBRARY_PATH", parent_str);
    }
    Ok(())
}

pub struct KotlinPatch {
    name: String,
    description: String,
    compatible_with: Vec<Compatibility>,
    enabled_by_default: bool,
    depends_on: Vec<String>,
    extension_dex: Vec<String>,
    options: Vec<OptionDeclaration>,
    patch_ref: jni::objects::GlobalRef,
    bundle_dir: PathBuf,
}

// SAFETY: KotlinPatch is Send+Sync because all fields are Send+Sync except `patch_ref` (GlobalRef),
// which wraps a JNI global reference — thread-safe by JNI spec, but the jni crate doesn't impl
// Sync due to the raw pointer. We only access it through JNI env calls which handle synchronization.
unsafe impl Send for KotlinPatch {}
unsafe impl Sync for KotlinPatch {}

impl Patch for KotlinPatch {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn compatible_with(&self) -> &[Compatibility] {
        &self.compatible_with
    }

    fn enabled_by_default(&self) -> bool {
        self.enabled_by_default
    }

    fn depends_on(&self) -> &[String] {
        &self.depends_on
    }

    fn extension_dex(&self) -> &[String] {
        &self.extension_dex
    }

    fn options(&self) -> &[OptionDeclaration] {
        &self.options
    }

    fn execute(&self, ctx: &mut PatchContext) -> Result<()> {
        call_patch_method(ctx, &self.patch_ref, &self.bundle_dir, "execute")
    }

    fn after_dependents(&self, ctx: &mut PatchContext) -> Result<()> {
        call_patch_method(ctx, &self.patch_ref, &self.bundle_dir, "afterDependents")
    }
}

fn call_patch_method(
    ctx: &mut PatchContext,
    patch_ref: &jni::objects::GlobalRef,
    bundle_dir: &Path,
    method_name: &str,
) -> Result<()> {
    let jvm = get_or_init_jvm()?;
    let mut env = jvm
        .attach_current_thread()
        .map_err(|e| jvm_err(format!("attach thread: {e}")))?;

    let _guard = CtxGuard::enter(ctx);
    BUNDLE_DIR.with(|bd| {
        *bd.borrow_mut() = Some(bundle_dir.to_path_buf());
    });

    let result = invoke_patch_method(&mut env, patch_ref.as_obj(), method_name);

    BUNDLE_DIR.with(|bd| *bd.borrow_mut() = None);
    // _guard drops here: nulls CTX_PTR, clears handles/xml/elements

    result
}

fn invoke_patch_method(
    env: &mut jni::JNIEnv<'_>,
    patch: &JObject<'_>,
    method_name: &str,
) -> Result<()> {
    let patch_class = env
        .call_method(patch, "getClass", "()Ljava/lang/Class;", &[])
        .and_then(|v| v.l())
        .map_err(|e| jvm_err(format!("patch.getClass(): {e}")))?;
    let loader = env
        .call_method(&patch_class, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])
        .and_then(|v| v.l())
        .map_err(|e| jvm_err(format!("patch class loader: {e}")))?;
    let runtime_name = env
        .new_string("dev.stitch.patch.PatchRuntime")
        .map_err(|e| jvm_err(format!("PatchRuntime name: {e}")))?;
    let runtime_class = env
        .call_method(
            &loader,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[JValue::Object(&runtime_name)],
        )
        .and_then(|v| v.l())
        .map_err(|e| jvm_err(format!("loadClass(PatchRuntime): {e}")))?;
    let runtime = env
        .new_object(
            jni::objects::JClass::from(runtime_class),
            "()V",
            &[],
        )
        .map_err(|e| jvm_err(format!("construct PatchRuntime: {e}")))?;

    env.call_method(
        patch,
        method_name,
        "(Ldev/stitch/patch/PatchRuntime;)V",
        &[JValue::Object(&runtime)],
    )
        .map_err(|e| {
            if env.exception_check().unwrap_or(false) {
                env.exception_describe().ok();
                env.exception_clear().ok();
            }
            jvm_err(format!("{method_name}(PatchRuntime): {e}"))
        })?;

    if env.exception_check().unwrap_or(false) {
        env.exception_describe().ok();
        env.exception_clear().ok();
        return Err(jvm_err(format!(
            "Kotlin patch threw an exception in {method_name}(PatchRuntime)"
        )));
    }

    Ok(())
}

fn create_class_loader<'a>(
    env: &mut jni::JNIEnv<'a>,
    jar_paths: &[PathBuf],
) -> Result<JObject<'a>> {
    let url_class = env
        .find_class("java/net/URL")
        .map_err(|e| jvm_err(format!("find URL class: {e}")))?;
    let url_array = env
        .new_object_array(jar_paths.len() as i32, &url_class, &JObject::null())
        .map_err(|e| jvm_err(format!("URL array: {e}")))?;

    for (i, jar_path) in jar_paths.iter().enumerate() {
        let jar_url = format!("file:{}", jar_path.display());
        let url_str = env
            .new_string(&jar_url)
            .map_err(|e| jvm_err(format!("new_string: {e}")))?;
        let url_obj = env
            .call_static_method(
                "java/net/URI",
                "create",
                "(Ljava/lang/String;)Ljava/net/URI;",
                &[JValue::Object(&url_str)],
            )
            .map_err(|e| jvm_err(format!("URI.create: {e}")))?
            .l()
            .map_err(|e| jvm_err(format!("URI obj: {e}")))?;
        let url = env
            .call_method(&url_obj, "toURL", "()Ljava/net/URL;", &[])
            .map_err(|e| jvm_err(format!("URI.toURL: {e}")))?
            .l()
            .map_err(|e| jvm_err(format!("URL obj: {e}")))?;
        env.set_object_array_element(&url_array, i as i32, &url)
            .map_err(|e| jvm_err(format!("set URL[{i}]: {e}")))?;
    }

    env.new_object(
        "java/net/URLClassLoader",
        "([Ljava/net/URL;)V",
        &[JValue::Object(&url_array)],
    )
    .map_err(|e| jvm_err(format!("URLClassLoader: {e}")))
}

fn read_string_field(env: &mut jni::JNIEnv<'_>, obj: &JObject<'_>, getter: &str) -> Result<String> {
    let val = env
        .call_method(obj, getter, "()Ljava/lang/String;", &[])
        .map_err(|e| jvm_err(format!("{getter}(): {e}")))?
        .l()
        .map_err(|e| jvm_err(format!("{getter} obj: {e}")))?;
    let jstr: jni::objects::JString = val.into();
    env.get_string(&jstr)
        .map(|s| s.into())
        .map_err(|e| jvm_err(format!("{getter} string: {e}")))
}

fn read_bool_field(env: &mut jni::JNIEnv<'_>, obj: &JObject<'_>, getter: &str) -> Result<bool> {
    env.call_method(obj, getter, "()Z", &[])
        .map_err(|e| jvm_err(format!("{getter}(): {e}")))?
        .z()
        .map_err(|e| jvm_err(format!("{getter} bool: {e}")))
}

fn read_string_list(
    env: &mut jni::JNIEnv<'_>,
    obj: &JObject<'_>,
    getter: &str,
) -> Result<Vec<String>> {
    let list = env
        .call_method(obj, getter, "()Ljava/util/List;", &[])
        .map_err(|e| jvm_err(format!("{getter}(): {e}")))?
        .l()
        .map_err(|e| jvm_err(format!("{getter} obj: {e}")))?;

    let size = env
        .call_method(&list, "size", "()I", &[])
        .map_err(|e| jvm_err(format!("list.size(): {e}")))?
        .i()
        .map_err(|e| jvm_err(format!("size int: {e}")))? as usize;

    let mut result = Vec::with_capacity(size);
    for i in 0..size {
        let elem = env
            .call_method(
                &list,
                "get",
                "(I)Ljava/lang/Object;",
                &[JValue::Int(i as i32)],
            )
            .map_err(|e| jvm_err(format!("list.get({i}): {e}")))?
            .l()
            .map_err(|e| jvm_err(format!("list elem: {e}")))?;
        let jstr: jni::objects::JString = elem.into();
        let s: String = env
            .get_string(&jstr)
            .map(|s| s.into())
            .map_err(|e| jvm_err(format!("list string: {e}")))?;
        if !s.is_empty() {
            result.push(s);
        }
    }
    Ok(result)
}

fn read_optional_string_list(
    env: &mut jni::JNIEnv<'_>,
    obj: &JObject<'_>,
    getter: &str,
) -> Result<Option<Vec<String>>> {
    let list = env
        .call_method(obj, getter, "()Ljava/util/List;", &[])
        .map_err(|e| jvm_err(format!("{getter}(): {e}")))?
        .l()
        .map_err(|e| jvm_err(format!("{getter} obj: {e}")))?;
    if list.is_null() {
        return Ok(None);
    }

    let size = env
        .call_method(&list, "size", "()I", &[])
        .map_err(|e| jvm_err(format!("list.size(): {e}")))?
        .i()
        .map_err(|e| jvm_err(format!("size int: {e}")))? as usize;

    let mut result = Vec::with_capacity(size);
    for i in 0..size {
        let elem = env
            .call_method(
                &list,
                "get",
                "(I)Ljava/lang/Object;",
                &[JValue::Int(i as i32)],
            )
            .map_err(|e| jvm_err(format!("list.get({i}): {e}")))?
            .l()
            .map_err(|e| jvm_err(format!("list elem: {e}")))?;
        let jstr: jni::objects::JString = elem.into();
        let s: String = env
            .get_string(&jstr)
            .map(|s| s.into())
            .map_err(|e| jvm_err(format!("list string: {e}")))?;
        result.push(s);
    }
    Ok(Some(result))
}

fn read_optional_string_field(
    env: &mut jni::JNIEnv<'_>,
    obj: &JObject<'_>,
    getter: &str,
) -> Result<Option<String>> {
    let val = env
        .call_method(obj, getter, "()Ljava/lang/String;", &[])
        .map_err(|e| jvm_err(format!("{getter}(): {e}")))?
        .l()
        .map_err(|e| jvm_err(format!("{getter} obj: {e}")))?;
    if val.is_null() {
        return Ok(None);
    }
    let jstr: jni::objects::JString = val.into();
    env.get_string(&jstr)
        .map(|s| Some(s.into()))
        .map_err(|e| jvm_err(format!("{getter} string: {e}")))
}

fn read_optional_bool_field(
    env: &mut jni::JNIEnv<'_>,
    obj: &JObject<'_>,
    getter: &str,
) -> Result<Option<bool>> {
    let val = env
        .call_method(obj, getter, "()Ljava/lang/Boolean;", &[])
        .map_err(|e| jvm_err(format!("{getter}(): {e}")))?
        .l()
        .map_err(|e| jvm_err(format!("{getter} obj: {e}")))?;
    if val.is_null() {
        return Ok(None);
    }
    env.call_method(&val, "booleanValue", "()Z", &[])
        .map_err(|e| jvm_err(format!("{getter}.booleanValue(): {e}")))?
        .z()
        .map(Some)
        .map_err(|e| jvm_err(format!("{getter} bool: {e}")))
}

fn read_optional_long_field(
    env: &mut jni::JNIEnv<'_>,
    obj: &JObject<'_>,
    getter: &str,
) -> Result<Option<i64>> {
    let val = env
        .call_method(obj, getter, "()Ljava/lang/Long;", &[])
        .map_err(|e| jvm_err(format!("{getter}(): {e}")))?
        .l()
        .map_err(|e| jvm_err(format!("{getter} obj: {e}")))?;
    if val.is_null() {
        return Ok(None);
    }
    env.call_method(&val, "longValue", "()J", &[])
        .map_err(|e| jvm_err(format!("{getter}.longValue(): {e}")))?
        .j()
        .map(Some)
        .map_err(|e| jvm_err(format!("{getter} long: {e}")))
}

fn read_optional_double_field(
    env: &mut jni::JNIEnv<'_>,
    obj: &JObject<'_>,
    getter: &str,
) -> Result<Option<f64>> {
    let val = env
        .call_method(obj, getter, "()Ljava/lang/Double;", &[])
        .map_err(|e| jvm_err(format!("{getter}(): {e}")))?
        .l()
        .map_err(|e| jvm_err(format!("{getter} obj: {e}")))?;
    if val.is_null() {
        return Ok(None);
    }
    env.call_method(&val, "doubleValue", "()D", &[])
        .map_err(|e| jvm_err(format!("{getter}.doubleValue(): {e}")))?
        .d()
        .map(Some)
        .map_err(|e| jvm_err(format!("{getter} double: {e}")))
}

fn read_compatibility_list(
    env: &mut jni::JNIEnv<'_>,
    obj: &JObject<'_>,
) -> Result<Vec<Compatibility>> {
    let list = env
        .call_method(obj, "getCompatibleWith", "()Ljava/util/List;", &[])
        .map_err(|e| jvm_err(format!("getCompatibleWith(): {e}")))?
        .l()
        .map_err(|e| jvm_err(format!("getCompatibleWith obj: {e}")))?;

    let size = env
        .call_method(&list, "size", "()I", &[])
        .map_err(|e| jvm_err(format!("compat.size(): {e}")))?
        .i()
        .map_err(|e| jvm_err(format!("compat size int: {e}")))? as usize;

    let mut result = Vec::with_capacity(size);
    for i in 0..size {
        let elem = env
            .call_method(
                &list,
                "get",
                "(I)Ljava/lang/Object;",
                &[JValue::Int(i as i32)],
            )
            .map_err(|e| jvm_err(format!("compat.get({i}): {e}")))?
            .l()
            .map_err(|e| jvm_err(format!("compat elem: {e}")))?;
        let package = read_string_field(env, &elem, "getName")?;
        let versions = read_string_list(env, &elem, "getVersions")?;
        result.push(Compatibility::with_versions(package, versions));
    }
    Ok(result)
}

fn read_option_type(
    env: &mut jni::JNIEnv<'_>,
    obj: &JObject<'_>,
) -> Result<crate::options::OptionType> {
    let kind = env
        .call_method(obj, "getType", "()Ldev/stitch/patch/PatchOptionType;", &[])
        .map_err(|e| jvm_err(format!("getType(): {e}")))?
        .l()
        .map_err(|e| jvm_err(format!("getType obj: {e}")))?;
    let name = read_string_field(env, &kind, "name")?;
    match name.as_str() {
        "STRING" => Ok(crate::options::OptionType::String),
        "BOOL" => Ok(crate::options::OptionType::Bool),
        "INT" => Ok(crate::options::OptionType::Int),
        "FLOAT" => Ok(crate::options::OptionType::Float),
        "STRING_LIST" => Ok(crate::options::OptionType::StringList),
        "PATH" => Ok(crate::options::OptionType::Path),
        other => Err(jvm_err(format!("unknown PatchOptionType {other}"))),
    }
}

fn read_option_declarations(
    env: &mut jni::JNIEnv<'_>,
    obj: &JObject<'_>,
) -> Result<Vec<OptionDeclaration>> {
    let list = env
        .call_method(obj, "getOptions", "()Ljava/util/List;", &[])
        .map_err(|e| jvm_err(format!("getOptions(): {e}")))?
        .l()
        .map_err(|e| jvm_err(format!("getOptions obj: {e}")))?;

    let size = env
        .call_method(&list, "size", "()I", &[])
        .map_err(|e| jvm_err(format!("options.size(): {e}")))?
        .i()
        .map_err(|e| jvm_err(format!("options size int: {e}")))? as usize;

    let mut result = Vec::with_capacity(size);
    for i in 0..size {
        let elem = env
            .call_method(
                &list,
                "get",
                "(I)Ljava/lang/Object;",
                &[JValue::Int(i as i32)],
            )
            .map_err(|e| jvm_err(format!("options.get({i}): {e}")))?
            .l()
            .map_err(|e| jvm_err(format!("option elem: {e}")))?;

        let option_type = read_option_type(env, &elem)?;
        let default_value = match option_type {
            crate::options::OptionType::String | crate::options::OptionType::Path => {
                read_optional_string_field(env, &elem, "getDefaultString")?.map(|value| {
                    if matches!(option_type, crate::options::OptionType::Path) {
                        crate::options::OptionValue::Path(value.into())
                    } else {
                        crate::options::OptionValue::String(value)
                    }
                })
            }
            crate::options::OptionType::Bool => {
                read_optional_bool_field(env, &elem, "getDefaultBool")?
                    .map(crate::options::OptionValue::Bool)
            }
            crate::options::OptionType::Int => {
                read_optional_long_field(env, &elem, "getDefaultInt")?
                    .map(crate::options::OptionValue::Int)
            }
            crate::options::OptionType::Float => {
                read_optional_double_field(env, &elem, "getDefaultFloat")?
                    .map(crate::options::OptionValue::Float)
            }
            crate::options::OptionType::StringList => {
                read_optional_string_list(env, &elem, "getDefaultStringList")?
                    .map(crate::options::OptionValue::StringList)
            }
        };

        result.push(OptionDeclaration {
            key: read_string_field(env, &elem, "getKey")?,
            title: read_string_field(env, &elem, "getTitle")?,
            description: read_string_field(env, &elem, "getDescription")?,
            option_type,
            default_value,
            valid_values: read_optional_string_list(env, &elem, "getValidValues")?,
            required: read_bool_field(env, &elem, "getRequired")?,
        });
    }
    Ok(result)
}

fn scan_class_names(jar_paths: &[PathBuf]) -> Vec<String> {
    let mut class_names = Vec::new();
    for jar_path in jar_paths {
        let file = match std::fs::File::open(jar_path) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let mut archive = match zip::ZipArchive::new(file) {
            Ok(a) => a,
            Err(_) => continue,
        };
        for i in 0..archive.len() {
            let entry = match archive.by_index(i) {
                Ok(e) => e,
                Err(_) => continue,
            };
            let name = entry.name().to_string();
            if name.ends_with(".class") && !name.contains('$') && !name.starts_with("META-INF/") {
                let class_name = name
                    .strip_suffix(".class")
                    .map(|s| s.replace('/', "."))
                    .unwrap_or_default();
                if !class_name.is_empty() {
                    class_names.push(class_name);
                }
            }
        }
    }
    class_names
}

fn is_patch_type(env: &mut jni::JNIEnv<'_>, obj: &JObject<'_>, patch_class: &JObject<'_>) -> bool {
    let obj_class = match env.get_object_class(obj) {
        Ok(c) => c,
        Err(_) => return false,
    };
    env.call_method(
        patch_class,
        "isAssignableFrom",
        "(Ljava/lang/Class;)Z",
        &[JValue::Object(&obj_class)],
    )
    .and_then(|v| v.z())
    .unwrap_or(false)
}

fn collect_patch_obj<'a>(
    env: &mut jni::JNIEnv<'a>,
    patch_obj: &JObject<'a>,
    bundle_dir: &Path,
    bundle_extensions: &[PathBuf],
) -> Result<KotlinPatch> {
    let name = read_string_field(env, patch_obj, "getName")?;
    let description = read_string_field(env, patch_obj, "getDescription")?;
    let enabled = read_bool_field(env, patch_obj, "getEnabled")?;
    let deps = read_string_list(env, patch_obj, "getDependencies")?;
    let compat = read_compatibility_list(env, patch_obj)?;
    let mut ext_dex = read_string_list(env, patch_obj, "getExtensionDex")?
        .into_iter()
        .map(|path| bundle_dir.join(path).to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    ext_dex.extend(
        bundle_extensions
            .iter()
            .map(|path| path.to_string_lossy().into_owned()),
    );
    ext_dex.sort();
    ext_dex.dedup();
    let options = read_option_declarations(env, patch_obj)?;

    let patch_global = env
        .new_global_ref(patch_obj)
        .map_err(|e| jvm_err(format!("global ref: {e}")))?;

    Ok(KotlinPatch {
        name,
        description,
        compatible_with: compat,
        enabled_by_default: enabled,
        depends_on: deps,
        extension_dex: ext_dex,
        options,
        patch_ref: patch_global,
        bundle_dir: bundle_dir.to_path_buf(),
    })
}

pub fn load_kotlin_patches(
    jar_paths: &[PathBuf],
    bundle_dir: &Path,
    bundle_extensions: &[PathBuf],
) -> Result<Vec<Box<dyn Patch>>> {
    let class_names = scan_class_names(jar_paths);
    if class_names.is_empty() {
        return Ok(Vec::new());
    }

    let jvm = get_or_init_jvm()?;
    let mut env = jvm
        .attach_current_thread_permanently()
        .map_err(|e| jvm_err(format!("attach thread: {e}")))?;

    let loader = create_class_loader(&mut env, jar_paths)?;

    register_jni_natives(&mut env, &loader)?;

    let patch_iface_name = env
        .new_string("dev.stitch.patch.StitchPatch")
        .map_err(|e| jvm_err(format!("new_string: {e}")))?;
    let patch_class = env
        .call_method(
            &loader,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[JValue::Object(&patch_iface_name)],
        )
        .map_err(|e| jvm_err(format!("loadClass(StitchPatch): {e}")))?
        .l()
        .map_err(|e| jvm_err(format!("StitchPatch class: {e}")))?;

    let modifier_class = env
        .find_class("java/lang/reflect/Modifier")
        .map_err(|e| jvm_err(format!("find Modifier: {e}")))?;

    let mut patches: Vec<Box<dyn Patch>> = Vec::new();

    for class_name in &class_names {
        let name_j = match env.new_string(class_name) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let cls = match env
            .call_method(
                &loader,
                "loadClass",
                "(Ljava/lang/String;)Ljava/lang/Class;",
                &[JValue::Object(&name_j)],
            )
            .and_then(|v| v.l())
        {
            Ok(c) => c,
            Err(_) => {
                if env.exception_check().unwrap_or(false) {
                    env.exception_clear().ok();
                }
                continue;
            }
        };

        // Scan public static fields for StitchPatch instances.
        let fields = match env
            .call_method(&cls, "getFields", "()[Ljava/lang/reflect/Field;", &[])
            .and_then(|v| v.l())
        {
            Ok(f) => f,
            Err(_) => {
                if env.exception_check().unwrap_or(false) {
                    env.exception_clear().ok();
                }
                continue;
            }
        };

        let fields_arr: JObjectArray = fields.into();
        let field_count = env.get_array_length(&fields_arr).unwrap_or(0);
        for fi in 0..field_count {
            let field = match env.get_object_array_element(&fields_arr, fi) {
                Ok(f) => f,
                Err(_) => continue,
            };
            let mods = env
                .call_method(&field, "getModifiers", "()I", &[])
                .and_then(|v| v.i())
                .unwrap_or(0);
            let is_public = env
                .call_static_method(&modifier_class, "isPublic", "(I)Z", &[JValue::Int(mods)])
                .and_then(|v| v.z())
                .unwrap_or(false);
            let is_static = env
                .call_static_method(&modifier_class, "isStatic", "(I)Z", &[JValue::Int(mods)])
                .and_then(|v| v.z())
                .unwrap_or(false);
            if !is_public || !is_static {
                continue;
            }
            let value = match env
                .call_method(
                    &field,
                    "get",
                    "(Ljava/lang/Object;)Ljava/lang/Object;",
                    &[JValue::Object(&JObject::null())],
                )
                .and_then(|v| v.l())
            {
                Ok(v) if !v.is_null() => v,
                _ => continue,
            };
            if is_patch_type(&mut env, &value, &patch_class) {
                if let Ok(p) = collect_patch_obj(&mut env, &value, bundle_dir, bundle_extensions) {
                    if !p.name.is_empty() {
                        patches.push(Box::new(p));
                    }
                }
            }
        }

        // Scan public static no-arg methods returning StitchPatch.
        let methods = match env
            .call_method(&cls, "getMethods", "()[Ljava/lang/reflect/Method;", &[])
            .and_then(|v| v.l())
        {
            Ok(m) => m,
            Err(_) => {
                if env.exception_check().unwrap_or(false) {
                    env.exception_clear().ok();
                }
                continue;
            }
        };

        let methods_arr: JObjectArray = methods.into();
        let method_count = env.get_array_length(&methods_arr).unwrap_or(0);
        for mi in 0..method_count {
            let method = match env.get_object_array_element(&methods_arr, mi) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let mods = env
                .call_method(&method, "getModifiers", "()I", &[])
                .and_then(|v| v.i())
                .unwrap_or(0);
            let is_public = env
                .call_static_method(&modifier_class, "isPublic", "(I)Z", &[JValue::Int(mods)])
                .and_then(|v| v.z())
                .unwrap_or(false);
            let is_static = env
                .call_static_method(&modifier_class, "isStatic", "(I)Z", &[JValue::Int(mods)])
                .and_then(|v| v.z())
                .unwrap_or(false);
            if !is_public || !is_static {
                continue;
            }
            let param_count = env
                .call_method(&method, "getParameterCount", "()I", &[])
                .and_then(|v| v.i())
                .unwrap_or(-1);
            if param_count != 0 {
                continue;
            }
            let ret_type = match env
                .call_method(&method, "getReturnType", "()Ljava/lang/Class;", &[])
                .and_then(|v| v.l())
            {
                Ok(rt) => rt,
                Err(_) => continue,
            };
            let ret_is_patch = env
                .call_method(
                    &patch_class,
                    "isAssignableFrom",
                    "(Ljava/lang/Class;)Z",
                    &[JValue::Object(&ret_type)],
                )
                .and_then(|v| v.z())
                .unwrap_or(false);
            if !ret_is_patch {
                continue;
            }
            let value = match env
                .call_method(
                    &method,
                    "invoke",
                    "(Ljava/lang/Object;[Ljava/lang/Object;)Ljava/lang/Object;",
                    &[
                        JValue::Object(&JObject::null()),
                        JValue::Object(&JObject::null()),
                    ],
                )
                .and_then(|v| v.l())
            {
                Ok(v) if !v.is_null() => v,
                _ => {
                    if env.exception_check().unwrap_or(false) {
                        env.exception_clear().ok();
                    }
                    continue;
                }
            };
            if let Ok(p) = collect_patch_obj(&mut env, &value, bundle_dir, bundle_extensions) {
                if !p.name.is_empty() {
                    patches.push(Box::new(p));
                }
            }
        }
    }

    Ok(patches)
}

#[cfg(test)]
mod tests {
    use super::with_ctx;

    #[test]
    #[should_panic(expected = "patch context is not active")]
    fn with_ctx_requires_active_context() {
        with_ctx(|_| ());
    }
}
