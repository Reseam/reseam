// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! An option's declaration and text, read from the option object a patch lists.

use jni::objects::{JObject, JValueOwned};
use jni::{Env, jni_sig, jni_str};

use super::jni_values::{boolean, list_strings, object, optional_strings, string, string_of};
use super::jvm::jvm_err;
use super::patch_text::{StringTables, Text};
use crate::error::Result;
use crate::options::{OptionDeclaration, OptionType, OptionValue};

/// An option and the text of its title and description.
pub(in crate::bridge) struct ReadOption {
    pub(in crate::bridge) declaration: OptionDeclaration,
    pub(in crate::bridge) title: Text,
    pub(in crate::bridge) description: Option<Text>,
}

pub(in crate::bridge) fn read_option(
    env: &mut Env<'_>,
    tables: &mut StringTables<'_>,
    option: &JObject<'_>,
) -> Result<ReadOption> {
    let title = tables
        .optional(env, option, "getTitle")?
        .ok_or_else(|| jvm_err("getTitle() returned null"))?;
    let description = tables.optional(env, option, "getDescription")?;
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
    let declaration = OptionDeclaration {
        key: string(env, option, "getKey")?,
        title: title.source.clone(),
        description: description
            .as_ref()
            .map(|it| it.source.clone())
            .unwrap_or_default(),
        option_type,
        default_value,
        valid_values: optional_strings(env, option, "getValidValues")?,
        required: boolean(env, option, "getRequired")?,
    };
    Ok(ReadOption {
        declaration,
        title,
        description,
    })
}
