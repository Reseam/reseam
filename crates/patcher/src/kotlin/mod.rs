pub(crate) mod bytecode;
pub(crate) mod convert;
mod log_host;
mod manifest;
mod options;
mod resources;
pub mod types;
mod xml;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use jni::objects::{JObject, JObjectArray, JValue};
use jni::{InitArgsBuilder, JavaVM, JNIVersion};
use stitch_apk::AxmlDocument;
use stitch_apk::stitch_dex::{DexFile, EncodedMethod};

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
    pub(crate) static CTX_PTR: Cell<*mut ()> = const { Cell::new(std::ptr::null_mut()) };
    pub(crate) static HANDLES: RefCell<HandleTable> = RefCell::new(HandleTable::default());
    pub(crate) static XML_DOCUMENTS: RefCell<Vec<Option<(AxmlDocument, String)>>> = RefCell::new(Vec::new());
    pub(crate) static PENDING_ELEMENTS: RefCell<Vec<xml::PendingElement>> = RefCell::new(Vec::new());
    pub(crate) static BUNDLE_DIR: RefCell<Option<PathBuf>> = RefCell::new(None);
}

include!(concat!(env!("OUT_DIR"), "/jni_natives.rs"));

fn register_jni_natives(
    env: &mut jni::JNIEnv<'_>,
    loader: &JObject<'_>,
) -> Result<()> {
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
        let ctx = unsafe { &mut *(ptr as *mut PatchContext<'_>) };
        f(ctx)
    })
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

pub(crate) fn get_method_ref(dex: &DexFile, mh: MethodHandle) -> &EncodedMethod {
    try_get_method_ref(dex, mh).unwrap_or_else(|| {
        static EMPTY: std::sync::OnceLock<EncodedMethod> = std::sync::OnceLock::new();
        EMPTY.get_or_init(|| EncodedMethod {
            method: stitch_apk::stitch_dex::MethodIdx(0),
            access_flags: stitch_apk::stitch_dex::AccessFlags::empty(),
            code: None,
        })
    })
}

pub(crate) fn get_method_mut(dex: &mut DexFile, mh: MethodHandle) -> &mut EncodedMethod {
    try_get_method_mut(dex, mh).unwrap_or_else(|| {
        // Only reached on invalid handle — leak a dummy to avoid panicking
        Box::leak(Box::new(EncodedMethod {
            method: stitch_apk::stitch_dex::MethodIdx(0),
            access_flags: stitch_apk::stitch_dex::AccessFlags::empty(),
            code: None,
        }))
    })
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
                libloading_jvm(&jvm_lib).map_err(|e| format!("{e}"))?;
            }

            let lib_path = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .unwrap_or_else(|| PathBuf::from("."));
            let jvm_args = InitArgsBuilder::new()
                .version(JNIVersion::V8)
                .option("-Xmx256m")
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

unsafe fn libloading_jvm(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| jvm_err("no parent dir for libjvm"))?;
    let parent_str = parent.to_str().unwrap_or("");
    if let Ok(current) = std::env::var("LD_LIBRARY_PATH") {
        if !current.contains(parent_str) {
            std::env::set_var(
                "LD_LIBRARY_PATH",
                format!("{parent_str}:{current}"),
            );
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
    options: Vec<OptionDeclaration>,
    patch_ref: jni::objects::GlobalRef,
    bundle_dir: PathBuf,
}

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

    fn options(&self) -> &[OptionDeclaration] {
        &self.options
    }

    fn execute(&self, ctx: &mut PatchContext) -> Result<()> {
        let jvm = get_or_init_jvm()?;
        let mut env = jvm
            .attach_current_thread()
            .map_err(|e| jvm_err(format!("attach thread: {e}")))?;

        CTX_PTR.with(|cell| {
            cell.set(ctx as *mut PatchContext as *mut ());
        });
        BUNDLE_DIR.with(|bd| {
            *bd.borrow_mut() = Some(self.bundle_dir.clone());
        });
        HANDLES.with(|h| {
            *h.borrow_mut() = HandleTable::default();
        });
        XML_DOCUMENTS.with(|docs| {
            docs.borrow_mut().clear();
        });
        PENDING_ELEMENTS.with(|pe| {
            pe.borrow_mut().clear();
        });

        let result = execute_patch(&mut env, self.patch_ref.as_obj());

        CTX_PTR.with(|cell| cell.set(std::ptr::null_mut()));
        BUNDLE_DIR.with(|bd| *bd.borrow_mut() = None);
        HANDLES.with(|h| *h.borrow_mut() = HandleTable::default());
        XML_DOCUMENTS.with(|docs| docs.borrow_mut().clear());
        PENDING_ELEMENTS.with(|pe| pe.borrow_mut().clear());

        result
    }
}

fn execute_patch(env: &mut jni::JNIEnv<'_>, patch: &JObject<'_>) -> Result<()> {
    env.call_method(patch, "execute", "()V", &[])
        .map_err(|e| {
            if env.exception_check().unwrap_or(false) {
                env.exception_describe().ok();
                env.exception_clear().ok();
            }
            jvm_err(format!("execute(): {e}"))
        })?;

    if env.exception_check().unwrap_or(false) {
        env.exception_describe().ok();
        env.exception_clear().ok();
        return Err(jvm_err("Kotlin patch threw an exception"));
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

fn read_string_field(
    env: &mut jni::JNIEnv<'_>,
    obj: &JObject<'_>,
    getter: &str,
) -> Result<String> {
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

fn read_bool_field(
    env: &mut jni::JNIEnv<'_>,
    obj: &JObject<'_>,
    getter: &str,
) -> Result<bool> {
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
) -> Result<KotlinPatch> {
    let name = read_string_field(env, patch_obj, "getName")?;
    let description = read_string_field(env, patch_obj, "getDescription")?;
    let enabled = read_bool_field(env, patch_obj, "getEnabled")?;
    let deps = read_string_list(env, patch_obj, "getDependencies")?;
    let compat = read_string_list(env, patch_obj, "getCompatibleWith")?;

    let patch_global = env
        .new_global_ref(patch_obj)
        .map_err(|e| jvm_err(format!("global ref: {e}")))?;

    Ok(KotlinPatch {
        name,
        description,
        compatible_with: compat
            .iter()
            .map(|s| Compatibility::package(s))
            .collect(),
        enabled_by_default: enabled,
        depends_on: deps,
        options: Vec::new(),
        patch_ref: patch_global,
        bundle_dir: bundle_dir.to_path_buf(),
    })
}

pub fn load_kotlin_patches(
    jar_paths: &[PathBuf],
    bundle_dir: &Path,
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
                if let Ok(p) = collect_patch_obj(&mut env, &value, bundle_dir) {
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
                .call_method(
                    &method,
                    "getReturnType",
                    "()Ljava/lang/Class;",
                    &[],
                )
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
                    &[JValue::Object(&JObject::null()), JValue::Object(&JObject::null())],
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
            if let Ok(p) = collect_patch_obj(&mut env, &value, bundle_dir) {
                if !p.name.is_empty() {
                    patches.push(Box::new(p));
                }
            }
        }
    }

    Ok(patches)
}
