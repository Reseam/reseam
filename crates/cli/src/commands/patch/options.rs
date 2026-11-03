// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;

use anyhow::{Context, Result, anyhow, ensure};
use reseam_model::{OptionDeclaration, OptionType, PatchMetadata, PatchSelection};
use reseam_sdk::PreparedInspection;

/// `selection` with each `PATCH.KEY=VALUE` in `options` set on the patch it
/// names. Each flag for a list option adds one item.
pub(super) fn with_options(
    mut selection: PatchSelection,
    options: &[String],
    prepared: &PreparedInspection,
) -> Result<PatchSelection> {
    let mut texts: BTreeMap<(String, &str), (&OptionDeclaration, Vec<&str>)> = BTreeMap::new();
    for raw in options {
        let invalid = || anyhow!("invalid option '{raw}': expected PATCH.KEY=VALUE");
        let (lhs, value) = raw.split_once('=').ok_or_else(invalid)?;
        let (patch, key) = option_target(prepared, lhs)?;
        let reference = patch.spec.reference().to_string();
        let declaration = patch
            .spec
            .options
            .iter()
            .find(|declaration| declaration.key == key)
            .with_context(|| format!("unknown option '{key}' for patch '{reference}'"))?;
        let (_, given) = texts
            .entry((reference, key))
            .or_insert_with(|| (declaration, Vec::new()));
        ensure!(
            given.is_empty() || declaration.option_type == OptionType::StringList,
            "option {lhs} is given more than once"
        );
        given.push(value);
    }
    for ((reference, key), (declaration, given)) in texts {
        let value = declaration
            .parse(&given.join("\n"))
            .with_context(|| format!("invalid --option {reference}.{key}"))?;
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
