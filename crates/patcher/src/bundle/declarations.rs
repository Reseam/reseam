// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::PATCH_INDEX;
use super::index::Declaration;
use crate::error::{BundleError, Result};
#[cfg(feature = "kotlin")]
use std::collections::BTreeSet;
use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

/// The patch declarations of `jars`, by the class that declares them.
pub(crate) fn declarations(jars: &[PathBuf]) -> Result<BTreeMap<String, Vec<Declaration>>> {
    let mut classes = HashSet::new();
    let mut found: BTreeMap<String, Vec<Declaration>> = BTreeMap::new();
    for jar in jars {
        let mut archive = ZipArchive::new(File::open(jar)?)?;
        classes.extend(
            archive
                .file_names()
                .filter_map(|name| name.strip_suffix(".class"))
                .map(|name| name.replace('/', ".")),
        );
        for declaration in index(jar, &mut archive)? {
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

/// The payload names of the jars among `jars` that declare each patch, by patch id.
#[cfg(feature = "kotlin")]
pub(crate) fn declaring_jars(jars: &[PathBuf]) -> Result<BTreeMap<String, BTreeSet<String>>> {
    let mut declaring: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for jar in jars {
        let name = jar
            .file_name()
            .and_then(|name| name.to_str())
            .expect("payload files are named in UTF-8, as staging checks");
        for declaration in index(jar, &mut ZipArchive::new(File::open(jar)?)?)? {
            declaring
                .entry(declaration.id)
                .or_default()
                .insert(name.to_owned());
        }
    }
    Ok(declaring)
}

/// The declarations `jar`'s patch index lists.
fn index(jar: &Path, archive: &mut ZipArchive<File>) -> Result<Vec<Declaration>> {
    let mut entry =
        archive
            .by_name(PATCH_INDEX)
            .map_err(|source| BundleError::MissingDeclarationIndex {
                jar: jar.to_owned(),
                source,
            })?;
    let mut bytes = Vec::new();
    entry
        .by_ref()
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(BundleError::DeclarationIndexTooLarge(jar.to_owned()).into());
    }
    Ok(
        serde_json::from_slice(&bytes).map_err(|source| BundleError::DeclarationIndex {
            jar: jar.to_owned(),
            source,
        })?,
    )
}
