// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The payload a bundle is packed from, copied into a snapshot and hashed so
//! what is signed is what gets written.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Seek;
use std::path::{Path, PathBuf};

use reseam_storage::{ScratchDir, copy_hashed};
use zip::ZipArchive;

use super::PATCH_INDEX;
use super::payload::{PayloadKind, payload_kind};
use crate::error::{BundleError, JarPart, Result};

/// A payload file in the snapshot, with the SHA-256 it was staged with.
pub(super) struct Staged {
    pub(super) path: PathBuf,
    pub(super) hash: [u8; 32],
}

/// Copies the payload of `dir` into `snapshot`: adjacent `.jar` and `.dex`
/// files, and everything under `resources/`.
pub(super) fn stage(dir: &Path, snapshot: &ScratchDir) -> Result<BTreeMap<String, Staged>> {
    let mut staged = BTreeMap::new();
    for (name, path) in payload_paths(dir)? {
        let payload_error = |source| BundleError::Payload {
            name: name.clone(),
            source,
        };
        let mut file = File::open(&path).map_err(payload_error)?;
        if payload_kind(&name)? == PayloadKind::Jar {
            check_universal_jar(&name, &mut file)?;
            file.rewind()?;
        }
        let target = snapshot.path().join(&name);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let hash = copy_hashed(&mut file, &mut File::create(&target)?).map_err(payload_error)?;
        staged.insert(name, Staged { path: target, hash });
    }
    Ok(staged)
}

fn payload_paths(dir: &Path) -> Result<BTreeMap<String, PathBuf>> {
    let mut paths = BTreeMap::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = utf8_name(&entry)?;
        if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("jar" | "dex")
        ) {
            payload_kind(&name)?;
            if !entry.file_type()?.is_file() {
                return Err(BundleError::NotAFile(path).into());
            }
            paths.insert(name, path);
        } else if name == "resources" {
            resource_paths(&path, "resources", &mut paths)?;
        }
    }
    if paths.is_empty() {
        return Err(BundleError::NoPayload(dir.to_owned()).into());
    }
    Ok(paths)
}

fn resource_paths(dir: &Path, prefix: &str, paths: &mut BTreeMap<String, PathBuf>) -> Result<()> {
    if !std::fs::symlink_metadata(dir)?.is_dir() {
        return Err(BundleError::NotADirectory(dir.to_owned()).into());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = format!("{prefix}/{}", utf8_name(&entry)?);
        payload_kind(&name)?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            resource_paths(&entry.path(), &name, paths)?;
        } else if kind.is_file() {
            paths.insert(name, entry.path());
        } else {
            return Err(BundleError::NotAFile(entry.path()).into());
        }
    }
    Ok(())
}

fn utf8_name(entry: &std::fs::DirEntry) -> Result<String> {
    entry
        .file_name()
        .into_string()
        .map_err(|name| BundleError::PayloadName(name.to_string_lossy().into_owned()).into())
}

/// Fails unless the jar carries JVM classes, `classes.dex` and its patch
/// declaration index, so one bundle runs on the desktop JVM and on ART.
fn check_universal_jar(name: &str, file: &mut File) -> Result<()> {
    let archive = ZipArchive::new(file).map_err(|source| BundleError::Jar {
        name: name.to_owned(),
        source,
    })?;
    let (mut classes, mut dex, mut index) = (false, false, false);
    for entry in archive.file_names() {
        classes |= Path::new(entry)
            .extension()
            .is_some_and(|extension| extension == "class")
            && !entry.starts_with("META-INF/");
        dex |= entry == "classes.dex";
        index |= entry == PATCH_INDEX;
    }
    let missing = [
        (classes, JarPart::Classes),
        (dex, JarPart::Dex),
        (index, JarPart::Index),
    ]
    .into_iter()
    .find_map(|(present, part)| (!present).then_some(part));
    match missing {
        Some(missing) => Err(BundleError::IncompleteJar {
            name: name.to_owned(),
            missing,
        }
        .into()),
        None => Ok(()),
    }
}
