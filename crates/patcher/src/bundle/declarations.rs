// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::PATCH_INDEX;
use super::index::Declaration;
use crate::error::{BundleError, Result};
use std::collections::{BTreeMap, HashSet};
use std::io::Read;
use std::path::PathBuf;

pub(crate) fn declarations(jars: &[PathBuf]) -> Result<BTreeMap<String, Vec<Declaration>>> {
    let mut classes = HashSet::new();
    let mut found: BTreeMap<String, Vec<Declaration>> = BTreeMap::new();
    for jar in jars {
        let mut archive = zip::ZipArchive::new(std::fs::File::open(jar)?)?;
        classes.extend(
            archive
                .file_names()
                .filter_map(|name| name.strip_suffix(".class"))
                .map(|name| name.replace('/', ".")),
        );
        let mut entry = archive.by_name(PATCH_INDEX).map_err(|source| {
            BundleError::MissingDeclarationIndex {
                jar: jar.clone(),
                source,
            }
        })?;
        let mut bytes = Vec::new();
        entry
            .by_ref()
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err(BundleError::DeclarationIndexTooLarge(jar.clone()).into());
        }
        let index: Vec<Declaration> =
            serde_json::from_slice(&bytes).map_err(|source| BundleError::DeclarationIndex {
                jar: jar.clone(),
                source,
            })?;
        for declaration in index {
            let members = found.entry(declaration.class_name.clone()).or_default();
            if !members.contains(&declaration) {
                members.push(declaration);
            }
        }
    }
    for class in found.keys() {
        if !classes.contains(class)
            || found[class]
                .iter()
                .any(|declaration| !classes.contains(&declaration.owner))
        {
            return Err(BundleError::UndeclaredClass(class.clone()).into());
        }
    }
    Ok(found)
}
