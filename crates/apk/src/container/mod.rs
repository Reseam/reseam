// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod metadata;

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use tracing::instrument;

use metadata::{ContainerMarker, check_metadata, read_metadata};

use crate::apk_file::order_components;
use crate::entry::MANIFEST_ENTRY;
use crate::error::{Result, invalid};
use crate::zip::reader::{self, Archive};
use crate::{ApkComponent, ApkFile};
use reseam_dex::{Loading, ParseOptions};
use reseam_storage::ScratchDir;

const INFO_JSON: &str = "info.json";
const MANIFEST_JSON: &str = "manifest.json";

pub use reseam_model::ContainerFormat;

/// Opens an APKM or XAPK container as a session over the APKs it holds,
/// extracted under their entry names into a scratch directory the session
/// removes when dropped. `None` when `path` is not a container.
#[instrument(level = "info", skip_all, fields(path = %path.display()))]
pub fn open_container(
    path: &Path,
    options: ParseOptions,
) -> Result<Option<(ContainerFormat, ApkFile)>> {
    let mut archive = reader::open_archive(path)?;
    let Some(format) = detect_format(&mut archive, path)? else {
        return Ok(None);
    };
    let apk_entries = apk_entries(&archive)?;
    let metadata_name = match format {
        ContainerFormat::Apkm => INFO_JSON,
        ContainerFormat::Xapk => MANIFEST_JSON,
    };
    let metadata = read_metadata(&mut archive, metadata_name)?;
    if metadata
        .as_ref()
        .is_some_and(|info| !info.expansions.is_empty())
    {
        return Err(invalid(
            "container",
            "XAPK expansion files are not supported",
        ));
    }
    let scratch = ScratchDir::new("apk-container")?;
    let mut components: Vec<_> = apk_entries
        .par_iter()
        .map_with(archive.clone(), |archive, entry| {
            let path = extract_entry(archive, entry, scratch.path())?;
            ApkComponent::open(&path, Loading::Deferred)
        })
        .collect::<Result<_>>()?;
    order_components(&mut components)?;
    if let Some(metadata) = &metadata {
        check_metadata(metadata, &components, apk_entries.len())?;
    }
    let apk = ApkFile::from_components(components, options, Some(scratch))?;
    Ok(Some((format, apk)))
}

fn detect_format(archive: &mut Archive, path: &Path) -> Result<Option<ContainerFormat>> {
    // APKs can themselves contain JSON metadata and embedded APK assets.
    if reader::contains(archive, MANIFEST_ENTRY) {
        return Ok(None);
    }
    for (entry, format) in [
        (INFO_JSON, ContainerFormat::Apkm),
        (MANIFEST_JSON, ContainerFormat::Xapk),
    ] {
        if reader::contains(archive, entry) {
            let data = reader::read_entry(archive, entry)?;
            if serde_json::from_slice::<ContainerMarker>(&data)
                .is_ok_and(|info| info.version(format).is_some())
            {
                return Ok(Some(format));
            }
        }
    }
    Ok(extension_format(path))
}

fn apk_entries(archive: &Archive) -> Result<Vec<String>> {
    let mut entries = Vec::new();
    for name in archive.file_names() {
        let extension = Path::new(name).extension();
        if extension.is_some_and(|ext| ext.eq_ignore_ascii_case("obb")) {
            return Err(invalid(
                "container",
                "XAPK expansion files are not supported",
            ));
        }
        if extension.is_some_and(|ext| ext.eq_ignore_ascii_case("apk")) {
            // Only a single portable filename may be joined to the scratch path.
            if name.contains(['/', '\\', ':']) {
                return Err(invalid(
                    "container",
                    format!("unexpected APK entries inside folders or invalid filename: {name}"),
                ));
            }
            entries.push(name.to_string());
        }
    }
    if entries.is_empty() {
        return Err(invalid("container", "archive contains no APK entries"));
    }
    entries.sort();
    Ok(entries)
}

fn extension_format(path: &Path) -> Option<ContainerFormat> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "apkm" => Some(ContainerFormat::Apkm),
        "xapk" => Some(ContainerFormat::Xapk),
        _ => None,
    }
}

fn extract_entry(archive: &mut Archive, name: &str, dir: &Path) -> Result<PathBuf> {
    let path = dir.join(name);
    let mut entry = archive.by_name(name)?;
    let size = entry.size();
    let mut output = File::create(&path)?;
    let written = io::copy(&mut entry, &mut output)?;
    if written != size {
        return Err(invalid(
            "container",
            format!("truncated APK entry {name}: expected {size} bytes, got {written}"),
        ));
    }
    Ok(path)
}
