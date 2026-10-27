// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::{Context, Result, anyhow, ensure};
use reseam_model::{OptionDeclaration, OptionType, OptionValue, PatchMetadata, PatchSelection};
use reseam_sdk::PreparedInspection;

/// `selection` with each `PATCH.KEY=VALUE` in `options` set on the patch it names.
pub(super) fn with_options(
    mut selection: PatchSelection,
    options: &[String],
    prepared: &PreparedInspection,
) -> Result<PatchSelection> {
    for raw in options {
        let invalid = || anyhow!("invalid option '{raw}': expected PATCH.KEY=VALUE");
        let (lhs, value) = raw.split_once('=').ok_or_else(invalid)?;
        let (patch, key) = option_target(prepared, lhs)?;
        let reference = patch.spec.reference.to_string();
        let declaration = patch
            .spec
            .options
            .iter()
            .find(|declaration| declaration.key == key)
            .with_context(|| format!("unknown option '{key}' for patch '{reference}'"))?;
        let value =
            parse_option(declaration, value).with_context(|| format!("invalid --option {raw}"))?;
        selection
            .options
            .entry(reference)
            .or_default()
            .insert(key.to_owned(), value);
    }
    Ok(selection)
}

/// Splits `PATCH.KEY` at the last dot that leaves a known patch, since patch
/// IDs and names contain dots themselves.
fn option_target<'p, 'a>(
    prepared: &'p PreparedInspection,
    lhs: &'a str,
) -> Result<(&'p PatchMetadata, &'a str)> {
    for (separator, _) in lhs.rmatch_indices('.') {
        let (selector, key) = (&lhs[..separator], &lhs[separator + 1..]);
        ensure!(
            !selector.is_empty() && !key.is_empty(),
            "invalid option '{lhs}': expected PATCH.KEY"
        );
        if let Some(patch) = prepared.resolve_patch(selector)? {
            return Ok((patch, key));
        }
    }
    Err(anyhow!(
        "unknown patch in option '{lhs}'; expected PATCH.KEY (use <bundle>/<id>, an ID, or an unambiguous name)"
    ))
}

fn parse_option(declaration: &OptionDeclaration, raw: &str) -> Result<OptionValue> {
    Ok(match declaration.option_type {
        OptionType::String => OptionValue::Text(raw.to_owned()),
        OptionType::Bool => OptionValue::Bool(raw.parse().context("expected bool")?),
        OptionType::Int => OptionValue::Int(raw.parse().context("expected int")?),
        OptionType::Float => OptionValue::Float(raw.parse().context("expected float")?),
        OptionType::StringList => OptionValue::TextList(
            raw.split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .collect(),
        ),
        OptionType::Path => OptionValue::Path(raw.to_owned()),
    })
}
