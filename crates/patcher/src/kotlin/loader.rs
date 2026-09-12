// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Finds `ReseamPatch` objects in a bundle's jars and reads their metadata
//! through JNI reflection.

use std::path::{Path, PathBuf};

use jni::objects::{GlobalRef, JObject, JObjectArray, JValue};
use jni::JNIEnv;
use tracing::warn;

use super::jvm::{self, jvm_err, string_of};
use super::patch::{load_class, KotlinPatch};
use crate::error::Result;
use crate::options::{OptionDeclaration, OptionType, OptionValue};
use crate::patch::{is_slug, CompatiblePackage, Patch, PatchSpec};

const PATCH_INTERFACE: &str = "app.reseam.patch.ReseamPatch";
const EXTERNAL_PATCH: &str = "app.reseam.patch.ExternalPatch";
const NATIVE_CLASS: &str = "app.reseam.patch.Native";

include!(concat!(env!("OUT_DIR"), "/jni_natives.rs"));

/// A patch object and where it was declared, before its metadata is read.
struct Found {
    object: GlobalRef,
    /// `<package>.<property>` identity, independent of the display name.
    declaration: String,
}

pub fn load_patches(
    jars: &[PathBuf],
    bundle_dir: &Path,
    bundle: &str,
) -> Result<Vec<Box<dyn Patch>>> {
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
        let external_class = load_class(env, &loader, EXTERNAL_PATCH)?;
        let mut found: Vec<Found> = Vec::new();
        for name in &class_names {
            let Ok(class) = load_class(env, &loader, name) else {
                jvm::clear_pending_exception(env);
                continue;
            };
            let package = name.rsplit_once('.').map_or("", |(package, _)| package);
            for (object, member) in patch_objects(env, &class, &patch_class) {
                // A generated reference to another bundle's patch, not a declaration.
                if is_instance(env, &object, &external_class) {
                    continue;
                }
                let declaration = if package.is_empty() {
                    member
                } else {
                    format!("{package}.{member}")
                };
                if let Some(seen) = found.iter_mut().find(|seen| {
                    env.is_same_object(seen.object.as_obj(), &object)
                        .unwrap_or(false)
                }) {
                    // Export aliases refer to one patch. Pick the same identity
                    // regardless of reflection or jar-entry enumeration order.
                    if declaration < seen.declaration {
                        seen.declaration = declaration;
                    }
                    continue;
                }
                found.push(Found {
                    object: env
                        .new_global_ref(&object)
                        .map_err(|e| jvm_err(format!("global ref: {e}")))?,
                    declaration,
                });
            }
        }
        found
            .iter()
            .map(|patch| {
                read_patch(env, patch, &found, &external_class, bundle_dir, bundle)
                    .map(|patch| Box::new(patch) as Box<dyn Patch>)
            })
            .collect()
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
/// public static no-argument methods, each with the property name it was
/// declared under. Members are selected by their declared type first, so
/// only classes that actually publish a patch get initialized.
fn patch_objects<'a>(
    env: &mut JNIEnv<'a>,
    class: &JObject<'_>,
    patch_class: &JObject<'_>,
) -> Vec<(JObject<'a>, String)> {
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
                if let Ok(name) = string(env, &field, "getName") {
                    found.push((value, name));
                }
            }
            _ => report_member_failure(env, &field),
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
            Ok(value) if !value.is_null() => {
                if let Ok(name) = string(env, &method, "getName") {
                    found.push((value, property_name(&name)));
                }
            }
            _ => report_member_failure(env, &method),
        }
    }
    found
}

/// A patch declaration that threw while initializing is skipped, but never
/// silently: the class initializer's failure is what the author needs to see.
fn report_member_failure(env: &mut JNIEnv<'_>, member: &JObject<'_>) {
    if let Some(exception) = jvm::take_pending_exception(env) {
        let name = string(env, member, "getName").unwrap_or_default();
        warn!(member = name, %exception, "patch declaration failed to initialize");
    }
}

/// `getFooBar` as the Kotlin property `fooBar`.
fn property_name(getter: &str) -> String {
    match getter.strip_prefix("get") {
        Some(rest) if rest.chars().next().is_some_and(char::is_uppercase) => {
            let mut chars = rest.chars();
            let first = chars.next().unwrap_or_default().to_lowercase();
            format!("{first}{}", chars.as_str())
        }
        _ => getter.to_string(),
    }
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

fn read_patch(
    env: &mut JNIEnv<'_>,
    found: &Found,
    all: &[Found],
    external_class: &JObject<'_>,
    bundle_dir: &Path,
    bundle: &str,
) -> Result<KotlinPatch> {
    let patch = found.object.as_obj();
    let name = optional_string(env, patch, "getName")?;
    let hidden = name.is_none() || boolean(env, patch, "getHidden")?;
    let id = found.declaration.clone();
    let dependencies = objects(env, patch, "getDependencies")?
        .into_iter()
        .map(|dependency| {
            if is_instance(env, &dependency, external_class) {
                let other = string(env, &dependency, "getBundle")?;
                if !is_slug(&other) {
                    return Err(jvm_err(format!(
                        "patch {id} depends on a bundle named '{other}'; bundle names are lowercase letters, digits, and hyphens"
                    )));
                }
                return Ok(format!("{other}/{}", string(env, &dependency, "getId")?));
            }
            all.iter()
                .find(|candidate| {
                    env.is_same_object(candidate.object.as_obj(), &dependency)
                        .unwrap_or(false)
                })
                .map(|candidate| format!("{bundle}/{}", candidate.declaration))
                .ok_or_else(|| {
                    jvm_err(format!(
                        "patch {id} depends on a patch that is not declared as a public top-level value"
                    ))
                })
        })
        .collect::<Result<_>>()?;
    let compatibility = objects(env, patch, "getCompatibleWith")?
        .into_iter()
        .map(|entry| {
            Ok(CompatiblePackage {
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
        bundle: bundle.to_owned(),
        name: name.unwrap_or_else(|| id.clone()),
        id,
        hidden,
        description: string(env, patch, "getDescription")?,
        enabled_by_default: !hidden && boolean(env, patch, "getEnabled")?,
        dependencies,
        compatibility,
        options,
    };
    Ok(KotlinPatch {
        spec,
        object: found.object.clone(),
        bundle_dir: bundle_dir.to_path_buf(),
    })
}

fn read_option(env: &mut JNIEnv<'_>, option: &JObject<'_>) -> Result<OptionDeclaration> {
    let kind = object(env, option, "getKind", "()Lapp/reseam/patch/OptionKind;")?;
    let option_type = match string(env, &kind, "name")?.as_str() {
        "STRING" => OptionType::String,
        "BOOL" => OptionType::Bool,
        "INT" => OptionType::Int,
        "FLOAT" => OptionType::Float,
        "STRING_LIST" => OptionType::StringList,
        "PATH" => OptionType::Path,
        other => return Err(jvm_err(format!("unknown OptionKind {other}"))),
    };
    let default = object(env, option, "getDefault", "()Ljava/lang/Object;")?;
    let default_value = if default.is_null() {
        None
    } else {
        Some(match option_type {
            OptionType::String => OptionValue::String(
                string_of(env, default).map_err(|e| jvm_err(format!("default: {e}")))?,
            ),
            OptionType::Path => OptionValue::Path(
                string_of(env, default)
                    .map_err(|e| jvm_err(format!("default: {e}")))?
                    .into(),
            ),
            OptionType::Bool => OptionValue::Bool(
                env.call_method(&default, "booleanValue", "()Z", &[])
                    .and_then(|v| v.z())
                    .map_err(|e| jvm_err(format!("default: {e}")))?,
            ),
            OptionType::Int => OptionValue::Int(
                env.call_method(&default, "longValue", "()J", &[])
                    .and_then(|v| v.j())
                    .map_err(|e| jvm_err(format!("default: {e}")))?,
            ),
            OptionType::Float => OptionValue::Float(
                env.call_method(&default, "doubleValue", "()D", &[])
                    .and_then(|v| v.d())
                    .map_err(|e| jvm_err(format!("default: {e}")))?,
            ),
            OptionType::StringList => {
                OptionValue::StringList(list_strings(env, default, "default")?)
            }
        })
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

fn optional_string(
    env: &mut JNIEnv<'_>,
    target: &JObject<'_>,
    getter: &str,
) -> Result<Option<String>> {
    let value = object(env, target, getter, "()Ljava/lang/String;")?;
    if value.is_null() {
        return Ok(None);
    }
    string_of(env, value)
        .map(Some)
        .map_err(|e| jvm_err(format!("{getter}: {e}")))
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
    list_objects(env, list, getter).map(Some)
}

fn list_objects<'a>(
    env: &mut JNIEnv<'a>,
    list: JObject<'a>,
    what: &str,
) -> Result<Vec<JObject<'a>>> {
    let size = env
        .call_method(&list, "size", "()I", &[])
        .and_then(|v| v.i())
        .map_err(|e| jvm_err(format!("{what}.size(): {e}")))?;
    (0..size)
        .map(|i| {
            env.call_method(&list, "get", "(I)Ljava/lang/Object;", &[JValue::Int(i)])
                .and_then(|v| v.l())
                .map_err(|e| jvm_err(format!("{what}.get({i}): {e}")))
        })
        .collect()
}

fn list_strings<'a>(env: &mut JNIEnv<'a>, list: JObject<'a>, what: &str) -> Result<Vec<String>> {
    list_objects(env, list, what)?
        .into_iter()
        .map(|item| string_of(env, item).map_err(|e| jvm_err(format!("{what} element: {e}"))))
        .collect()
}

fn optional_strings(
    env: &mut JNIEnv<'_>,
    target: &JObject<'_>,
    getter: &str,
) -> Result<Option<Vec<String>>> {
    let list = object(env, target, getter, "()Ljava/util/List;")?;
    if list.is_null() {
        return Ok(None);
    }
    list_strings(env, list, getter).map(Some)
}

fn strings(env: &mut JNIEnv<'_>, target: &JObject<'_>, getter: &str) -> Result<Vec<String>> {
    optional_strings(env, target, getter).map(Option::unwrap_or_default)
}

#[cfg(test)]
mod tests {
    use super::property_name;

    #[test]
    fn getters_map_to_properties() {
        assert_eq!(property_name("getTelegramSettings"), "telegramSettings");
        assert_eq!(property_name("getURLPatch"), "uRLPatch");
        assert_eq!(property_name("settings"), "settings");
        assert_eq!(property_name("getter"), "getter");
    }
}
