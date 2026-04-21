// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use reseam_patcher::bundle::PatchBundle;
use reseam_patcher::engine::{ExecutionPlan, PatchSelection as EnginePatchSelection};
use reseam_patcher::options::{OptionDeclaration, OptionValue, PatchOptions};

use crate::dto::{InputOptionValue, PatchSelection};

pub fn parse_cli_option(option: &str) -> Result<(String, String, String)> {
    let (lhs, value) = option
        .split_once('=')
        .with_context(|| format!("invalid option '{option}': expected PATCH.KEY=VALUE"))?;
    let (patch_name, option_key) = lhs
        .split_once('.')
        .with_context(|| format!("invalid option '{option}': expected PATCH.KEY=VALUE"))?;
    if patch_name.is_empty() || option_key.is_empty() {
        bail!("invalid option '{option}': patch and key must be non-empty");
    }

    Ok((
        patch_name.to_string(),
        option_key.to_string(),
        value.to_string(),
    ))
}

pub fn selection_from_cli(
    enable: &[String],
    disable: &[String],
    option_args: &[String],
    bundle: &PatchBundle,
) -> Result<PatchSelection> {
    let mut selection = PatchSelection {
        enable: enable.to_vec(),
        disable: disable.to_vec(),
        options: Default::default(),
    };

    for raw in option_args {
        let (patch_name, option_key, value) = parse_cli_option(raw)?;
        let declaration = find_option_declaration(&bundle.patches, &patch_name, &option_key)?;
        let parsed = declaration
            .parse_value(&value)
            .with_context(|| format!("failed to parse --option {raw}"))?;
        selection
            .options
            .entry(patch_name)
            .or_default()
            .insert(option_key, InputOptionValue::from(&parsed));
    }

    Ok(selection)
}

pub fn compile_patch_selection(
    patches: &[Box<dyn reseam_patcher::patch::Patch>],
    selection: &PatchSelection,
) -> Result<EnginePatchSelection> {
    let mut compiled = EnginePatchSelection::new();

    for patch in &selection.enable {
        compiled.select_patch(patch.clone());
    }
    for patch in &selection.disable {
        compiled.disable_patch(patch.clone());
    }

    for (patch_name, options) in &selection.options {
        let mut patch_options = PatchOptions::new();
        for (option_key, value) in options {
            let declaration = find_option_declaration(patches, patch_name, option_key)?;
            patch_options.set(
                option_key.clone(),
                input_to_option_value(value, declaration)
                    .with_context(|| format!("invalid option {patch_name}.{option_key}"))?,
            );
        }
        compiled.set_patch_options(patch_name.clone(), patch_options);
    }

    Ok(compiled)
}

pub fn build_execution_plan(
    patches: &[Box<dyn reseam_patcher::patch::Patch>],
    selection: &PatchSelection,
) -> Result<ExecutionPlan> {
    compile_patch_selection(patches, selection)
}

fn input_to_option_value(
    value: &InputOptionValue,
    declaration: &OptionDeclaration,
) -> Result<OptionValue> {
    let value = match value {
        InputOptionValue::String(value) => OptionValue::String(value.clone()),
        InputOptionValue::Bool(value) => OptionValue::Bool(*value),
        InputOptionValue::Int(value) => OptionValue::Int(*value),
        InputOptionValue::Float(value) => OptionValue::Float(*value),
        InputOptionValue::StringList(value) => OptionValue::StringList(value.clone()),
        InputOptionValue::Path(value) => OptionValue::Path(PathBuf::from(value)),
    };
    declaration.validate_value(&value)?;
    Ok(value)
}

fn find_option_declaration<'a>(
    patches: &'a [Box<dyn reseam_patcher::patch::Patch>],
    patch_name: &str,
    option_key: &str,
) -> Result<&'a OptionDeclaration> {
    let patch = patches
        .iter()
        .find(|patch| patch.name() == patch_name)
        .with_context(|| format!("unknown patch '{patch_name}'"))?;
    patch
        .options()
        .iter()
        .find(|declaration| declaration.key.as_str() == option_key)
        .with_context(|| format!("unknown option '{option_key}' for patch '{patch_name}'"))
}
