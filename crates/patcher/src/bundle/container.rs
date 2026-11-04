// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The zip a bundle is stored in: the `mimetype` marker, the manifest, its
//! signature when it has one, then the payload.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::{BUNDLE_MIMETYPE, BundleKey};
use crate::error::{BundleError, Result};

pub(super) const MANIFEST_LIMIT: u64 = 16 * 1024 * 1024;

/// Opens the bundle at `path`, which must start with the bundle mimetype marker.
pub(super) fn open(path: &Path) -> Result<ZipArchive<File>> {
    let mut archive =
        ZipArchive::new(File::open(path)?).map_err(|source| BundleError::Archive {
            path: path.to_owned(),
            source,
        })?;
    if read_entry(&mut archive, "mimetype", BUNDLE_MIMETYPE.len() as u64)?
        != BUNDLE_MIMETYPE.as_bytes()
    {
        return Err(BundleError::Mimetype.into());
    }
    Ok(archive)
}

/// A control entry, which must not exceed `limit` bytes.
pub(super) fn read_entry(
    archive: &mut ZipArchive<File>,
    name: &'static str,
    limit: u64,
) -> Result<Vec<u8>> {
    let mut entry = archive
        .by_name(name)
        .map_err(|source| BundleError::MissingEntry { name, source })?;
    let mut buf = Vec::new();
    entry.by_ref().take(limit + 1).read_to_end(&mut buf)?;
    if entry.size() > limit || buf.len() as u64 > limit {
        return Err(BundleError::EntryTooLarge { name, limit }.into());
    }
    Ok(buf)
}

pub(super) fn deflated() -> SimpleFileOptions {
    SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)
}

/// Writes a bundle next to `out` and then moves it into place: the control
/// entries, with `key`'s signature over `manifest` when there is a key, and
/// the payload `write_payload` adds.
pub(super) fn write(
    out: &Path,
    manifest: &[u8],
    key: Option<&BundleKey>,
    write_payload: impl FnOnce(&mut ZipWriter<&mut File>) -> Result<()>,
) -> Result<()> {
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let parent = out
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    let mut zip = ZipWriter::new(temporary.as_file_mut());
    let signature = key.map(|key| (key.public_key(), key.sign(manifest)));
    let mut entries = vec![
        ("mimetype", stored, BUNDLE_MIMETYPE.as_bytes()),
        ("manifest.toml", deflated(), manifest),
    ];
    if let Some((public_key, signature)) = &signature {
        entries.push(("manifest.pubkey", stored, public_key.as_slice()));
        entries.push(("manifest.sig", stored, signature.as_slice()));
    }
    for (name, options, bytes) in entries {
        zip.start_file(name, options)?;
        zip.write_all(bytes)?;
    }
    write_payload(&mut zip)?;
    zip.finish()?;
    temporary.as_file().sync_all()?;
    temporary.persist(out).map_err(|error| error.error)?;
    Ok(())
}
