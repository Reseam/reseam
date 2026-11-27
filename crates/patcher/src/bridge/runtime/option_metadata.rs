// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! An option's declaration and text, read from the option object a patch lists.

use jni::objects::{JObject, JValueOwned};
use jni::{Env, jni_sig, jni_str};

use super::jni_values::{
    boolean, list_strings, map_list, object, optional_strings, string, string_of,
};
use super::jvm::jvm_err;
use super::patch_text::{StringTables, Text};
use crate::error::Result;
use crate::options::{OptionDeclaration, OptionPreset, OptionType, OptionValue};

/// An option and the text of its title and description.
pub(in crate::bridge) struct ReadOption {
    pub(in crate::bridge) declaration: OptionDeclaration,
    pub(in crate::bridge) title: Text,
    pub(in crate::bridge) description: Option<Text>,
    /// In the order of the declaration's presets.
    pub(in crate::bridge) preset_labels: Vec<Text>,
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
        Some(value_of(env, default, option_type)?)
    };
    let preset_list = object(env, option, "getPresets", "()Ljava/util/List;")?;
    let presets = map_list(env, &preset_list, "getPresets", |env, preset| {
        let label = tables
            .optional(env, preset, "getLabel")?
            .ok_or_else(|| jvm_err("OptionPreset.getLabel() returned null"))?;
        let value = object(env, preset, "getValue", "()Ljava/lang/Object;")?;
        Ok((label, value_of(env, value, option_type)?))
    })?;
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
        presets: presets
            .iter()
            .map(|(label, value)| OptionPreset {
                label: label.source.clone(),
                value: value.clone(),
            })
            .collect(),
        allows_custom: boolean(env, option, "getAllowsCustom")?,
    };
    Ok(ReadOption {
        declaration,
        title,
        description,
        preset_labels: presets.into_iter().map(|(label, _)| label).collect(),
    })
}

fn value_of(env: &mut Env<'_>, value: JObject<'_>, option_type: OptionType) -> Result<OptionValue> {
    Ok(match option_type {
        OptionType::String => OptionValue::Text(string_of(env, value)?),
        OptionType::Path => OptionValue::Path(string_of(env, value)?),
        OptionType::Bool => OptionValue::Bool(
            env.call_method(&value, jni_str!("booleanValue"), jni_sig!("()Z"), &[])
                .and_then(JValueOwned::z)?,
        ),
        OptionType::Int => OptionValue::Int(
            env.call_method(&value, jni_str!("longValue"), jni_sig!("()J"), &[])
                .and_then(JValueOwned::j)?,
        ),
        OptionType::Float => OptionValue::Float(
            env.call_method(&value, jni_str!("doubleValue"), jni_sig!("()D"), &[])
                .and_then(JValueOwned::d)?,
        ),
        OptionType::StringList => OptionValue::TextList(list_strings(env, &value, "value")?),
    })
}
