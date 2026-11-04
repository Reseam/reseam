// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Write;
use std::path::Path;

use reseam_storage::copy_hashed;
use zip::ZipArchive;

use super::CONTROL_ENTRIES;
use crate::error::{BundleError, Result};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PayloadKind {
    Jar,
    Dex,
    Resource,
}

/// Windows device names, reserved in every directory with any extension.
const RESERVED_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// What the payload entry `name` holds: patch jars and extension DEX at the
/// root, anything under `resources/`.
pub(super) fn payload_kind(name: &str) -> Result<PayloadKind> {
    if !name.split('/').all(is_portable) {
        return Err(BundleError::UnsafePath(name.to_owned()).into());
    }
    let extension = Path::new(name).extension();
    if name.starts_with("resources/") {
        Ok(PayloadKind::Resource)
    } else if !name.contains('/') && extension.is_some_and(|extension| extension == "jar") {
        Ok(PayloadKind::Jar)
    } else if !name.contains('/') && extension.is_some_and(|extension| extension == "dex") {
        Ok(PayloadKind::Dex)
    } else {
        Err(BundleError::UnsupportedPayload(name.to_owned()).into())
    }
}

/// Whether a path part means the same file on every host. Windows interprets
/// backslashes, drive prefixes and device names even in archives made on Unix.
fn is_portable(part: &str) -> bool {
    let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
    !(part.is_empty()
        || part == "."
        || part == ".."
        || part.contains(['\\', ':', '\0'])
        || part.ends_with(['.', ' '])
        || part.chars().any(char::is_control)
        || RESERVED_NAMES.contains(&stem.as_str()))
}

/// Copies each payload entry of `archive` to the writer `open` gives for it,
/// checking it against `declared`, the manifest's SHA-256 of each file by
/// name: an entry the manifest does not declare, one that differs from it, and
/// a declared file the archive lacks all fail. Returns the entries' names and
/// kinds, in archive order.
pub(super) fn copy_payload<W: Write>(
    archive: &mut ZipArchive<File>,
    declared: &BTreeMap<String, String>,
    mut open: impl FnMut(&str, PayloadKind) -> Result<W>,
) -> Result<Vec<(String, PayloadKind)>> {
    let mut remaining: BTreeSet<&str> = declared.keys().map(String::as_str).collect();
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let name = entry.name().to_string();
        if entry.is_dir() || CONTROL_ENTRIES.contains(&name.as_str()) {
            continue;
        }
        if !remaining.remove(name.as_str()) {
            return Err(BundleError::UndeclaredPayload(name).into());
        }
        let expected: [u8; 32] = hex::decode(&declared[&name])
            .ok()
            .and_then(|hash| hash.try_into().ok())
            .ok_or_else(|| BundleError::PayloadHash(name.clone()))?;
        let kind = payload_kind(&name)?;
        let hash = copy_hashed(&mut entry, &mut open(&name, kind)?).map_err(|source| {
            BundleError::Payload {
                name: name.clone(),
                source,
            }
        })?;
        if hash != expected {
            return Err(BundleError::HashMismatch(name).into());
        }
        entries.push((name, kind));
    }
    if !remaining.is_empty() {
        return Err(BundleError::MissingPayload.into());
    }
    Ok(entries)
}
