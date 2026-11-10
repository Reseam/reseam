// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! A patch's spec, read from the patch object its declaration returns.

use std::sync::Arc;

use jni::objects::{JClass, JObject, JValueOwned};
use jni::{Env, jni_sig, jni_str};

use super::callback::KotlinCallback;
use super::discovery::Found;
use super::jni_values::{boolean, map_list, object, string, strings};
use super::jvm::jvm_err;
use super::loader::PatchLoader;
use super::option_metadata::read_option;
use super::patch_text::StringTables;
use super::patch_translations::translations;
use crate::bundle::manifest::is_slug;
use crate::error::Result;
use crate::patch::{CompatiblePackage, Patch, PatchRef, PatchSpec};

pub(in crate::bridge) fn read_patch(
    env: &mut Env<'_>,
    found: &Found,
    all: &[Found],
    external_class: &JClass<'_>,
    loader: &Arc<PatchLoader>,
    bundle: &str,
    tables: &mut StringTables<'_>,
) -> Result<Patch> {
    let patch = found.object.as_ref();
    let name = tables.optional(env, patch, "getName")?;
    let description = tables.optional(env, patch, "getDescription")?;
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
    let options = map_list(env, &options_list, "options", |env, option| {
        read_option(env, tables, option)
    })?;
    let translations = translations(name.as_ref(), description.as_ref(), &options);
    let options = options.into_iter().map(|it| it.declaration).collect();
    let spec = PatchSpec {
        name: name.map_or_else(|| id.clone(), |name| name.source),
        bundle: bundle.to_owned(),
        id,
        hidden,
        description: description.map(|it| it.source).unwrap_or_default(),
        enabled_by_default: !hidden && boolean(env, patch, "getEnabled")?,
        dependencies,
        compatibility: compatibility.into_iter().collect(),
        options,
        rewrites_output: boolean(env, patch, "getRewritesOutput")?,
        translations,
    };
    let finalizes = boolean(env, patch, "getFinalizes")?;
    let callback = KotlinCallback::new(env.new_global_ref(&found.object)?, Arc::clone(loader));
    Ok(Patch::new(spec, finalizes, move |phase| {
        callback.invoke(phase)
    }))
}

fn version_codes(env: &mut Env<'_>, entry: &JObject<'_>) -> Result<Vec<i32>> {
    let list = object(env, entry, "getVersionCodes", "()Ljava/util/List;")?;
    map_list(env, &list, "getVersionCodes", |env, item| {
        env.call_method(item, jni_str!("intValue"), jni_sig!("()I"), &[])
            .and_then(JValueOwned::i)
            .map_err(|error| jvm_err(format!("getVersionCodes element: {error}")))
    })
}
