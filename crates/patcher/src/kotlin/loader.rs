// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Finds `ReseamPatch` objects in a bundle's jars and reads their metadata
//! through JNI reflection.

use std::path::{Path, PathBuf};

use jni::objects::{JObject, JObjectArray, JValue};
use jni::JNIEnv;

use super::jvm::{self, jvm_err, string_of};
use super::patch::{load_class, KotlinPatch};
use crate::error::Result;
use crate::options::{OptionDeclaration, OptionType, OptionValue};
use crate::patch::{Compatibility, Patch, PatchSpec};

const PATCH_INTERFACE: &str = "app.reseam.patch.ReseamPatch";
const NATIVE_CLASS: &str = "app.reseam.patch.Native";

include!(concat!(env!("OUT_DIR"), "/jni_natives.rs"));

pub fn load_patches(jars: &[PathBuf], bundle_dir: &Path) -> Result<Vec<Box<dyn Patch>>> {
    let class_names = class_names(jars);
    if class_names.is_empty() {
        return Ok(Vec::new());
    }
    let mut env = jvm::get_or_init()?
        .attach_current_thread_permanently()
        .map_err(|e| jvm_err(format!("attach thread: {e}")))?;
    jvm::with_frame(&mut env, |env| {
        let loader = create_class_loader(env, jars)?;
        let native = load_class(env, &loader, NATIVE_CLASS)?;
        env.register_native_methods(
            <&jni::objects::JClass>::from(&native),
            &jni_native_methods(),
        )
        .map_err(|e| jvm_err(format!("register natives: {e}")))?;
        let patch_class = load_class(env, &loader, PATCH_INTERFACE)?;
        let mut patches: Vec<Box<dyn Patch>> = Vec::new();
        for name in &class_names {
            let Ok(class) = load_class(env, &loader, name) else {
                jvm::clear_pending_exception(env);
                continue;
            };
            for object in patch_objects(env, &class, &patch_class) {
                let patch = read_patch(env, &object, bundle_dir)?;
                if !patch.name().is_empty() {
                    patches.push(Box::new(patch));
                }
            }
        }
        Ok(patches)
    })
}

/// Top-level class names in the jars, in `a.b.C` form.
fn class_names(jars: &[PathBuf]) -> Vec<String> {
    jars.iter()
        .filter_map(|jar| zip::ZipArchive::new(std::fs::File::open(jar).ok()?).ok())
        .flat_map(|archive| archive.file_names().map(str::to_string).collect::<Vec<_>>())
        .filter(|name| !name.contains('$') && !name.starts_with("META-INF/"))
        .filter_map(|name| Some(name.strip_suffix(".class")?.replace('/', ".")))
        .collect()
}

#[cfg(not(target_os = "android"))]
fn create_class_loader<'a>(env: &mut JNIEnv<'a>, jars: &[PathBuf]) -> Result<JObject<'a>> {
    let url_class = env
        .find_class("java/net/URL")
        .map_err(|e| jvm_err(format!("find URL class: {e}")))?;
    let urls = env
        .new_object_array(jars.len() as i32, &url_class, JObject::null())
        .map_err(|e| jvm_err(format!("URL array: {e}")))?;
    for (i, jar) in jars.iter().enumerate() {
        let path = env
            .new_string(jar.to_string_lossy().as_ref())
            .map_err(|e| jvm_err(format!("new_string: {e}")))?;
        let url = (|| -> jni::errors::Result<JObject<'_>> {
            let file = env.new_object(
                "java/io/File",
                "(Ljava/lang/String;)V",
                &[JValue::Object(&path)],
            )?;
            let uri = env
                .call_method(&file, "toURI", "()Ljava/net/URI;", &[])?
                .l()?;
            env.call_method(&uri, "toURL", "()Ljava/net/URL;", &[])?.l()
        })()
        .map_err(|e| jvm_err(format!("jar URL for {}: {e}", jar.display())))?;
        env.set_object_array_element(&urls, i as i32, &url)
            .map_err(|e| jvm_err(format!("set URL[{i}]: {e}")))?;
    }
    env.new_object(
        "java/net/URLClassLoader",
        "([Ljava/net/URL;)V",
        &[JValue::Object(&urls)],
    )
    .map_err(|e| jvm_err(format!("URLClassLoader: {e}")))
}

#[cfg(target_os = "android")]
fn create_class_loader<'a>(env: &mut JNIEnv<'a>, jars: &[PathBuf]) -> Result<JObject<'a>> {
    use reseam_apk::entry::dex_ordinal;

    for jar in jars {
        let has_dex = std::fs::File::open(jar)
            .ok()
            .and_then(|file| zip::ZipArchive::new(file).ok())
            .is_some_and(|archive| archive.file_names().any(|name| dex_ordinal(name).is_some()));
        if !has_dex {
            return Err(jvm_err(format!(
                "Android patch jar {} does not contain classes.dex; rebuild patch jars as universal JVM/Android jars",
                jar.display()
            )));
        }
    }
    let parent = match super::android_host::configured_class_loader(env).map_err(jvm_err)? {
        Some(loader) => loader,
        None => {
            let thread = env
                .call_static_method(
                    "java/lang/Thread",
                    "currentThread",
                    "()Ljava/lang/Thread;",
                    &[],
                )
                .and_then(|v| v.l())
                .map_err(|e| jvm_err(format!("Thread.currentThread(): {e}")))?;
            let loader = env
                .call_method(
                    &thread,
                    "getContextClassLoader",
                    "()Ljava/lang/ClassLoader;",
                    &[],
                )
                .and_then(|v| v.l())
                .map_err(|e| jvm_err(format!("Thread.contextClassLoader: {e}")))?;
            if loader.is_null() {
                return Err(jvm_err("Android context ClassLoader is null; install a DexClassLoader before loading patches"));
            }
            loader
        }
    };
    let dex_paths = jars
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(":");
    let optimized_dir = jars
        .first()
        .and_then(|jar| jar.parent())
        .map(|dir| dir.join("dex-cache"))
        .ok_or_else(|| jvm_err("no patch jars available for Android DexClassLoader"))?;
    std::fs::create_dir_all(&optimized_dir).map_err(|e| {
        jvm_err(format!(
            "create DexClassLoader optimized directory {}: {e}",
            optimized_dir.display()
        ))
    })?;
    let dex_path = env
        .new_string(dex_paths)
        .map_err(|e| jvm_err(format!("DexClassLoader dexPath: {e}")))?;
    let optimized_path = env
        .new_string(optimized_dir.to_string_lossy().as_ref())
        .map_err(|e| jvm_err(format!("DexClassLoader optimizedDirectory: {e}")))?;
    env.new_object(
        "dalvik/system/DexClassLoader",
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/ClassLoader;)V",
        &[
            JValue::Object(&dex_path),
            JValue::Object(&optimized_path),
            JValue::Object(&JObject::null()),
            JValue::Object(&parent),
        ],
    )
    .map_err(|e| jvm_err(format!("DexClassLoader: {e}")))
}

/// `ReseamPatch` instances exposed by `class` as public static fields or
/// public static no-argument methods. Members are selected by their declared
/// type first, so only classes that actually publish a patch get initialized.
fn patch_objects<'a>(
    env: &mut JNIEnv<'a>,
    class: &JObject<'_>,
    patch_class: &JObject<'_>,
) -> Vec<JObject<'a>> {
    let mut found = Vec::new();
    let members = |env: &mut JNIEnv<'a>, getter: &str, sig: &str| -> Vec<JObject<'a>> {
        let Ok(array) = env.call_method(class, getter, sig, &[]).and_then(|v| v.l()) else {
            jvm::clear_pending_exception(env);
            return Vec::new();
        };
        let array = JObjectArray::from(array);
        let len = env.get_array_length(&array).unwrap_or(0);
        let mut members = Vec::new();
        for i in 0..len {
            if let Ok(member) = env.get_object_array_element(&array, i) {
                if is_public_static(env, &member) {
                    members.push(member);
                }
            }
        }
        members
    };
    for field in members(env, "getFields", "()[Ljava/lang/reflect/Field;") {
        let holds_patch = env
            .call_method(&field, "getType", "()Ljava/lang/Class;", &[])
            .and_then(|v| v.l())
            .is_ok_and(|field_type| is_assignable(env, patch_class, &field_type));
        if !holds_patch {
            jvm::clear_pending_exception(env);
            continue;
        }
        let value = env
            .call_method(
                &field,
                "get",
                "(Ljava/lang/Object;)Ljava/lang/Object;",
                &[JValue::Object(&JObject::null())],
            )
            .and_then(|v| v.l());
        match value {
            Ok(value) if !value.is_null() && is_instance(env, &value, patch_class) => {
                found.push(value)
            }
            _ => jvm::clear_pending_exception(env),
        }
    }
    for method in members(env, "getMethods", "()[Ljava/lang/reflect/Method;") {
        let params = env
            .call_method(&method, "getParameterCount", "()I", &[])
            .and_then(|v| v.i())
            .unwrap_or(-1);
        let returns_patch = env
            .call_method(&method, "getReturnType", "()Ljava/lang/Class;", &[])
            .and_then(|v| v.l())
            .is_ok_and(|ret| is_assignable(env, patch_class, &ret));
        if params != 0 || !returns_patch {
            continue;
        }
        let value = env
            .call_method(
                &method,
                "invoke",
                "(Ljava/lang/Object;[Ljava/lang/Object;)Ljava/lang/Object;",
                &[
                    JValue::Object(&JObject::null()),
                    JValue::Object(&JObject::null()),
                ],
            )
            .and_then(|v| v.l());
        match value {
            Ok(value) if !value.is_null() => found.push(value),
            _ => jvm::clear_pending_exception(env),
        }
    }
    found
}

fn is_public_static(env: &mut JNIEnv<'_>, member: &JObject<'_>) -> bool {
    let Ok(modifiers) = env
        .call_method(member, "getModifiers", "()I", &[])
        .and_then(|v| v.i())
    else {
        return false;
    };
    let test = |env: &mut JNIEnv<'_>, name: &str| {
        env.call_static_method(
            "java/lang/reflect/Modifier",
            name,
            "(I)Z",
            &[JValue::Int(modifiers)],
        )
        .and_then(|v| v.z())
        .unwrap_or(false)
    };
    test(env, "isPublic") && test(env, "isStatic")
}

fn is_instance(env: &mut JNIEnv<'_>, object: &JObject<'_>, class: &JObject<'_>) -> bool {
    env.get_object_class(object)
        .is_ok_and(|object_class| is_assignable(env, class, &object_class))
}

fn is_assignable(env: &mut JNIEnv<'_>, class: &JObject<'_>, from: &JObject<'_>) -> bool {
    env.call_method(
        class,
        "isAssignableFrom",
        "(Ljava/lang/Class;)Z",
        &[JValue::Object(from)],
    )
    .and_then(|v| v.z())
    .unwrap_or(false)
}

fn read_patch(env: &mut JNIEnv<'_>, patch: &JObject<'_>, bundle_dir: &Path) -> Result<KotlinPatch> {
    let mut extension_dex: Vec<PathBuf> = strings(env, patch, "getExtensionDex")?
        .into_iter()
        .filter(|path| !path.is_empty())
        .map(|path| bundle_dir.join(path))
        .collect();
    extension_dex.sort();
    extension_dex.dedup();
    let compatibility = objects(env, patch, "getCompatibleWith")?
        .into_iter()
        .map(|entry| {
            Ok(Compatibility {
                package: string(env, &entry, "getName")?,
                versions: strings(env, &entry, "getVersions")?,
            })
        })
        .collect::<Result<_>>()?;
    let options = objects(env, patch, "getOptions")?
        .into_iter()
        .map(|option| read_option(env, &option))
        .collect::<Result<_>>()?;
    let spec = PatchSpec {
        id: string(env, patch, "getName")?,
        description: string(env, patch, "getDescription")?,
        enabled_by_default: boolean(env, patch, "getEnabled")?,
        dependencies: strings(env, patch, "getDependencies")?
            .into_iter()
            .filter(|dep| !dep.is_empty())
            .collect(),
        compatibility,
        options,
        extension_dex,
    };
    Ok(KotlinPatch {
        spec,
        object: env
            .new_global_ref(patch)
            .map_err(|e| jvm_err(format!("global ref: {e}")))?,
        bundle_dir: bundle_dir.to_path_buf(),
    })
}

fn read_option(env: &mut JNIEnv<'_>, option: &JObject<'_>) -> Result<OptionDeclaration> {
    let kind = object(
        env,
        option,
        "getType",
        "()Lapp/reseam/patch/PatchOptionType;",
    )?;
    let option_type = match string(env, &kind, "name")?.as_str() {
        "STRING" => OptionType::String,
        "BOOL" => OptionType::Bool,
        "INT" => OptionType::Int,
        "FLOAT" => OptionType::Float,
        "STRING_LIST" => OptionType::StringList,
        "PATH" => OptionType::Path,
        other => return Err(jvm_err(format!("unknown PatchOptionType {other}"))),
    };
    let default_value = match option_type {
        OptionType::String => {
            optional_string(env, option, "getDefaultString")?.map(OptionValue::String)
        }
        OptionType::Path => {
            optional_string(env, option, "getDefaultString")?.map(|p| OptionValue::Path(p.into()))
        }
        OptionType::Bool => boxed(
            env,
            option,
            "getDefaultBool",
            "Ljava/lang/Boolean;",
            |env, v| env.call_method(v, "booleanValue", "()Z", &[])?.z(),
        )?
        .map(OptionValue::Bool),
        OptionType::Int => boxed(
            env,
            option,
            "getDefaultInt",
            "Ljava/lang/Long;",
            |env, v| env.call_method(v, "longValue", "()J", &[])?.j(),
        )?
        .map(OptionValue::Int),
        OptionType::Float => boxed(
            env,
            option,
            "getDefaultFloat",
            "Ljava/lang/Double;",
            |env, v| env.call_method(v, "doubleValue", "()D", &[])?.d(),
        )?
        .map(OptionValue::Float),
        OptionType::StringList => {
            optional_strings(env, option, "getDefaultStringList")?.map(OptionValue::StringList)
        }
    };
    Ok(OptionDeclaration {
        key: string(env, option, "getKey")?,
        title: string(env, option, "getTitle")?,
        description: string(env, option, "getDescription")?,
        option_type,
        default_value,
        valid_values: optional_strings(env, option, "getValidValues")?,
        required: boolean(env, option, "getRequired")?,
    })
}

fn object<'a>(
    env: &mut JNIEnv<'a>,
    target: &JObject<'_>,
    getter: &str,
    sig: &str,
) -> Result<JObject<'a>> {
    env.call_method(target, getter, sig, &[])
        .and_then(|v| v.l())
        .map_err(|e| jvm_err(format!("{getter}(): {e}")))
}

/// A boxed getter (`Boolean`, `Long`, ...) unwrapped with `unbox`, or `None`
/// when it returns null.
fn boxed<T>(
    env: &mut JNIEnv<'_>,
    target: &JObject<'_>,
    getter: &str,
    sig: &str,
    unbox: impl FnOnce(&mut JNIEnv<'_>, &JObject<'_>) -> jni::errors::Result<T>,
) -> Result<Option<T>> {
    let value = object(env, target, getter, &format!("(){sig}"))?;
    if value.is_null() {
        return Ok(None);
    }
    unbox(env, &value)
        .map(Some)
        .map_err(|e| jvm_err(format!("{getter}: {e}")))
}

fn optional_string(
    env: &mut JNIEnv<'_>,
    target: &JObject<'_>,
    getter: &str,
) -> Result<Option<String>> {
    boxed(env, target, getter, "Ljava/lang/String;", |env, v| {
        string_of(env, env.new_local_ref(v)?)
    })
}

fn string(env: &mut JNIEnv<'_>, target: &JObject<'_>, getter: &str) -> Result<String> {
    optional_string(env, target, getter)?
        .ok_or_else(|| jvm_err(format!("{getter}() returned null")))
}

fn boolean(env: &mut JNIEnv<'_>, target: &JObject<'_>, getter: &str) -> Result<bool> {
    env.call_method(target, getter, "()Z", &[])
        .and_then(|v| v.z())
        .map_err(|e| jvm_err(format!("{getter}(): {e}")))
}

fn objects<'a>(
    env: &mut JNIEnv<'a>,
    target: &JObject<'_>,
    getter: &str,
) -> Result<Vec<JObject<'a>>> {
    optional_objects(env, target, getter).map(Option::unwrap_or_default)
}

/// The elements of a `java.util.List` getter, or `None` for a null list.
fn optional_objects<'a>(
    env: &mut JNIEnv<'a>,
    target: &JObject<'_>,
    getter: &str,
) -> Result<Option<Vec<JObject<'a>>>> {
    let list = object(env, target, getter, "()Ljava/util/List;")?;
    if list.is_null() {
        return Ok(None);
    }
    let size = env
        .call_method(&list, "size", "()I", &[])
        .and_then(|v| v.i())
        .map_err(|e| jvm_err(format!("{getter}().size(): {e}")))?;
    (0..size)
        .map(|i| {
            env.call_method(&list, "get", "(I)Ljava/lang/Object;", &[JValue::Int(i)])
                .and_then(|v| v.l())
                .map_err(|e| jvm_err(format!("{getter}().get({i}): {e}")))
        })
        .collect::<Result<_>>()
        .map(Some)
}

fn optional_strings(
    env: &mut JNIEnv<'_>,
    target: &JObject<'_>,
    getter: &str,
) -> Result<Option<Vec<String>>> {
    optional_objects(env, target, getter)?
        .map(|items| {
            items
                .into_iter()
                .map(|item| {
                    string_of(env, item).map_err(|e| jvm_err(format!("{getter}() element: {e}")))
                })
                .collect()
        })
        .transpose()
}

fn strings(env: &mut JNIEnv<'_>, target: &JObject<'_>, getter: &str) -> Result<Vec<String>> {
    optional_strings(env, target, getter).map(Option::unwrap_or_default)
}
