// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! A fingerprint per patch: the SHA-256 of the payload files of the module that
//! declares it and of every module that module uses. Hosts compare fingerprints
//! across releases to tell which patched apps a release changes, so a release
//! that rebuilds one app's modules leaves the other apps' fingerprints as they were.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

use super::modules::Modules;
use super::stage::Staged;
use crate::patch::PatchSpec;

/// The fingerprint of each patch in `catalog`, by patch id, as hex. `declaring_jars`
/// names the jars that declare each patch.
pub(super) fn fingerprints(
    modules: &Modules,
    payload: &BTreeMap<String, Staged>,
    declaring_jars: &BTreeMap<String, BTreeSet<String>>,
    catalog: &[PatchSpec],
) -> BTreeMap<String, String> {
    catalog
        .iter()
        .map(|patch| {
            let jars = declaring_jars
                .get(&patch.id)
                .expect("the catalog is loaded from the jars' declarations");
            let mut files: Vec<(&str, &[u8; 32])> = modules
                .used_by(jars.iter().map(String::as_str))
                .into_iter()
                .flat_map(|module| {
                    module.files.iter().map(move |file| {
                        let content = match &module.source {
                            Some(source) => &source.0,
                            None => &payload[file].hash,
                        };
                        (file.as_str(), content)
                    })
                })
                .collect();
            files.sort();
            let mut hasher = Sha256::new();
            for (file, content) in files {
                hasher.update((file.len() as u64).to_le_bytes());
                hasher.update(file);
                hasher.update(content);
            }
            (patch.id.clone(), hex::encode(hasher.finalize()))
        })
        .collect()
}
