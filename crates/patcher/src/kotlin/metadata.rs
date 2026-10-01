// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::jvm::{self, jvm_err, string_of};
use super::patch::KotlinCallback;
use crate::error::Result;
use crate::options::{OptionDeclaration, OptionType, OptionValue};
use crate::patch::{CompatiblePackage, Patch, PatchSpec, is_slug};
use jni::objects::{JClass, JObject, JValue, JValueOwned};
use jni::{Env, jni_sig, jni_str};
use std::sync::Arc;

use super::loader::{Found, PatchLoader};

pub(super) fn read_patch(
    env: &mut Env<'_>,
    found: &Found,
    all: &[Found],
    external_class: &JClass<'_>,
    loader: &Arc<PatchLoader>,
    bundle: &str,
) -> Result<Patch> {
    let patch = found.object.as_ref();
    let name = optional_string(env, patch, "getName")?;
    let hidden = name.is_none() || boolean(env, patch, "getHidden")?;
    let id = found.declaration.clone();
    let dependencies_list = object(env, patch, "getDependencies", "()Ljava/util/List;")?;
    let dependencies = map_list(
        env,
        &dependencies_list,
        "dependencies",
        |env, dependency| {
            if env.is_instance_of(dependency, external_class)? {
                let other = string(env, dependency, "getBundle")?;
                if !is_slug(&other) {
                    return Err(jvm_err(format!(
                        "patch {id} depends on a bundle named '{other}'; bundle names are lowercase letters, digits, and hyphens"
                    )));
                }
                return Ok(format!("{other}/{}", string(env, dependency, "getId")?));
            }
            for candidate in all {
                if env.is_same_object(candidate.object.as_ref(), dependency)? {
                    return Ok(format!("{bundle}/{}", candidate.declaration));
                }
            }
            Err(jvm_err(format!(
                "patch {id} depends on a patch that is not declared as a public top-level value"
            )))
        },
    )?;
    let compatibility_list = object(env, patch, "getCompatibleWith", "()Ljava/util/List;")?;
    let compatibility = map_list(env, &compatibility_list, "compatibility", |env, entry| {
        Ok(CompatiblePackage {
            package: string(env, entry, "getName")?,
            versions: strings(env, entry, "getVersions")?,
        })
    })?;
    let options_list = object(env, patch, "getOptions", "()Ljava/util/List;")?;
    let options = map_list(env, &options_list, "options", read_option)?;
    let spec = PatchSpec {
        bundle: bundle.to_owned(),
        name: name.unwrap_or_else(|| id.clone()),
        id,
        hidden,
        description: string(env, patch, "getDescription")?,
        enabled_by_default: !hidden && boolean(env, patch, "getEnabled")?,
        dependencies,
        compatibility: compatibility.into_iter().collect(),
        options,
    };
    let callback = KotlinCallback::new(env.new_global_ref(&found.object)?, Arc::clone(loader));
    Ok(Patch::new(spec, move |phase, context| {
        callback.invoke(phase, context)
    }))
}

fn read_option(env: &mut Env<'_>, option: &JObject<'_>) -> Result<OptionDeclaration> {
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
            OptionType::String => OptionValue::Text(string_of(env, default)?),
            OptionType::Path => OptionValue::Path(string_of(env, default)?),
            OptionType::Bool => OptionValue::Bool(
                env.call_method(&default, jni_str!("booleanValue"), jni_sig!("()Z"), &[])
                    .and_then(JValueOwned::z)?,
            ),
            OptionType::Int => OptionValue::Int(
                env.call_method(&default, jni_str!("longValue"), jni_sig!("()J"), &[])
                    .and_then(JValueOwned::j)?,
            ),
            OptionType::Float => OptionValue::Float(
                env.call_method(&default, jni_str!("doubleValue"), jni_sig!("()D"), &[])
                    .and_then(JValueOwned::d)?,
            ),
            OptionType::StringList => {
                OptionValue::TextList(list_strings(env, &default, "default")?)
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

pub(super) fn object<'a>(
    env: &mut Env<'a>,
    target: &JObject<'_>,
    getter: &str,
    sig: &str,
) -> Result<JObject<'a>> {
    env.call_method(
        target,
        jni::strings::JNIString::new(getter),
        jni::signature::RuntimeMethodSignature::from_str(sig)?.method_signature(),
        &[],
    )
    .and_then(JValueOwned::l)
    .map_err(|e| jvm_err(format!("{getter}(): {e}")))
}

fn optional_string(
    env: &mut Env<'_>,
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

pub(super) fn string(env: &mut Env<'_>, target: &JObject<'_>, getter: &str) -> Result<String> {
    optional_string(env, target, getter)?
        .ok_or_else(|| jvm_err(format!("{getter}() returned null")))
}

fn boolean(env: &mut Env<'_>, target: &JObject<'_>, getter: &str) -> Result<bool> {
    env.call_method(
        target,
        jni::strings::JNIString::new(getter),
        jni_sig!("()Z"),
        &[],
    )
    .and_then(JValueOwned::z)
    .map_err(|e| jvm_err(format!("{getter}(): {e}")))
}

fn map_list<T>(
    env: &mut Env<'_>,
    list: &JObject<'_>,
    what: &str,
    mut read: impl FnMut(&mut Env<'_>, &JObject<'_>) -> Result<T>,
) -> Result<Vec<T>> {
    if list.is_null() {
        return Err(jvm_err(format!("{what} returned null")));
    }
    let size = env
        .call_method(list, jni_str!("size"), jni_sig!("()I"), &[])
        .and_then(JValueOwned::i)
        .map_err(|error| jvm_err(format!("{what}.size(): {error}")))?;
    if size < 0 {
        return Err(jvm_err(format!("{what}.size() returned a negative size")));
    }
    (0..size)
        .map(|index| {
            jvm::with_frame(env, |env| {
                let item = env
                    .call_method(
                        list,
                        jni_str!("get"),
                        jni_sig!("(I)Ljava/lang/Object;"),
                        &[JValue::Int(index)],
                    )
                    .and_then(JValueOwned::l)
                    .map_err(|error| jvm_err(format!("{what}.get({index}): {error}")))?;
                if item.is_null() {
                    return Err(jvm_err(format!("{what}.get({index}) returned null")));
                }
                read(env, &item)
            })
        })
        .collect()
}

fn list_strings(env: &mut Env<'_>, list: &JObject<'_>, what: &str) -> Result<Vec<String>> {
    map_list(env, list, what, |env, item| {
        let item = env.new_local_ref(item)?;
        string_of(env, item).map_err(|error| jvm_err(format!("{what} element: {error}")))
    })
}

fn optional_strings(
    env: &mut Env<'_>,
    target: &JObject<'_>,
    getter: &str,
) -> Result<Option<Vec<String>>> {
    let list = object(env, target, getter, "()Ljava/util/List;")?;
    if list.is_null() {
        return Ok(None);
    }
    list_strings(env, &list, getter).map(Some)
}

fn strings(env: &mut Env<'_>, target: &JObject<'_>, getter: &str) -> Result<Vec<String>> {
    optional_strings(env, target, getter).map(Option::unwrap_or_default)
}
