// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! A patch's spec, read from the patch object its declaration returns.

use std::sync::Arc;

use jni::objects::{JClass, JObject, JValueOwned};
use jni::{Env, jni_sig, jni_str};

use super::callback::KotlinCallback;
use super::discovery::Found;
use super::jni_values::{
    boolean, list_strings, map_list, object, optional_string, optional_strings, string, string_of,
    strings,
};
use super::jvm::jvm_err;
use super::loader::PatchLoader;
use crate::bundle::manifest::is_slug;
use crate::error::Result;
use crate::options::{OptionDeclaration, OptionType, OptionValue};
use crate::patch::{CompatiblePackage, Patch, PatchRef, PatchSpec};

pub(in crate::bridge) fn read_patch(
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
                return Ok(PatchRef {
                    bundle: other,
                    id: string(env, dependency, "getId")?,
                });
            }
            for candidate in all {
                if env.is_same_object(candidate.object.as_ref(), dependency)? {
                    return Ok(PatchRef {
                        bundle: bundle.to_owned(),
                        id: candidate.declaration.clone(),
                    });
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
            version_codes: version_codes(env, entry)?,
        })
    })?;
    let options_list = object(env, patch, "getOptions", "()Ljava/util/List;")?;
    let options = map_list(env, &options_list, "options", read_option)?;
    let spec = PatchSpec {
        name: name.unwrap_or_else(|| id.clone()),
        reference: PatchRef {
            bundle: bundle.to_owned(),
            id,
        },
        hidden,
        description: string(env, patch, "getDescription")?,
        enabled_by_default: !hidden && boolean(env, patch, "getEnabled")?,
        dependencies,
        compatibility: compatibility.into_iter().collect(),
        options,
        rewrites_output: boolean(env, patch, "getRewritesOutput")?,
    };
    let finalizes = boolean(env, patch, "getFinalizes")?;
    let callback = KotlinCallback::new(env.new_global_ref(&found.object)?, Arc::clone(loader));
    Ok(Patch::new(spec, finalizes, move |phase| {
        callback.invoke(phase)
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

fn version_codes(env: &mut Env<'_>, entry: &JObject<'_>) -> Result<Vec<i32>> {
    let list = object(env, entry, "getVersionCodes", "()Ljava/util/List;")?;
    map_list(env, &list, "getVersionCodes", |env, item| {
        env.call_method(item, jni_str!("intValue"), jni_sig!("()I"), &[])
            .and_then(JValueOwned::i)
            .map_err(|error| jvm_err(format!("getVersionCodes element: {error}")))
    })
}
