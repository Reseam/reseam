// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! A fingerprint per patch: the hash of every declaration and bundle file the
//! patch can reach. Hosts compare fingerprints across releases to tell which
//! patched apps a release changes, so a release that edits one app's patches
//! leaves the fingerprints of every other app's patches as they were.
//!
//! The patch jars' `classes.dex` and the extension DEX files form a graph of
//! class headers and members, linked by what each refers to ([`graph`]). Patch
//! code runs in the engine and extension code in the app, so each side resolves
//! its own references, and a class name may mean a different class on each. A
//! member counts only when something reaches it ([`reach`]), because the shared
//! Kotlin library is shrunk to what every extension uses together and its
//! classes change with them.

mod graph;
mod reach;

use std::collections::{BTreeMap, HashSet};

use super::index::Declaration;
use super::stage::Staged;
use crate::error::Result;
use crate::patch::PatchSpec;

/// The fingerprint of each patch in `catalog`, by patch id, as hex. Several
/// declarations can hold one patch; the catalog names it by one of them.
pub(super) fn fingerprints(
    payload: &BTreeMap<String, Staged>,
    declarations: &BTreeMap<String, Vec<Declaration>>,
    catalog: &[PatchSpec],
) -> Result<BTreeMap<String, String>> {
    let graph = graph::Graph::read(payload)?;
    let ids: HashSet<&str> = catalog.iter().map(|patch| patch.id.as_str()).collect();
    let mut roots: BTreeMap<&str, Vec<&Declaration>> = BTreeMap::new();
    for declaration in declarations.values().flatten() {
        if ids.contains(declaration.id.as_str()) {
            roots.entry(&declaration.id).or_default().push(declaration);
        }
    }
    Ok(roots
        .into_iter()
        .map(|(id, roots)| {
            (
                id.to_owned(),
                hex::encode(reach::fingerprint(&graph, &roots)),
            )
        })
        .collect())
}
