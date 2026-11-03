// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod archive;
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

pub use archive::write_split_archive;
pub use reseam_model::ContainerFormat;

/// Opens an APKM, XAPK or APKS container as a session over the APKs it holds,
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
    let metadata = match format {
        ContainerFormat::Apkm => read_metadata(&mut archive, INFO_JSON)?,
        ContainerFormat::Xapk => read_metadata(&mut archive, MANIFEST_JSON)?,
        ContainerFormat::Apks => None,
    };
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
    for name in reader::entry_names(archive)? {
        let name = name.as_str();
        let extension = Path::new(name).extension();
        if extension.is_some_and(|ext| ext.eq_ignore_ascii_case("obb")) {
            return Err(invalid(
                "container",
                "XAPK expansion files are not supported",
            ));
        }
        if extension.is_some_and(|ext| ext.eq_ignore_ascii_case("apk")) {
            if !is_entry_name(name) {
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

/// Whether `name` can name an APK in a container: a single portable file
/// name, which extraction joins to its scratch directory.
fn is_entry_name(name: &str) -> bool {
    !name.contains(['/', '\\', ':'])
}

fn extension_format(path: &Path) -> Option<ContainerFormat> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "apkm" => Some(ContainerFormat::Apkm),
        "xapk" => Some(ContainerFormat::Xapk),
        "apks" => Some(ContainerFormat::Apks),
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
