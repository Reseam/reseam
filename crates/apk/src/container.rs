// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! ZIP containers that distribute a base APK with its splits as one file:
//! APKMirror's `.apkm` and APKPure's `.xapk`. Opening a container extracts
//! its APK entries, CRC-verified, into a scratch directory, so the rest of
//! the engine works on a plain split set.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::axml::AxmlDocument;
use crate::entry::MANIFEST_ENTRY;
use crate::error::{invalid, Result};
use crate::scratch::ScratchDir;
use crate::zip::reader::{self, Archive};

const INFO_JSON: &str = "info.json";
const MANIFEST_JSON: &str = "manifest.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerFormat {
    Apkm,
    Xapk,
}

impl ContainerFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Apkm => "apkm",
            Self::Xapk => "xapk",
        }
    }
}

#[derive(Debug, Deserialize)]
struct ContainerMetadata {
    #[serde(alias = "pname")]
    package_name: Option<String>,
    split_apks: Option<Vec<XapkSplitFile>>,
    #[serde(default)]
    expansions: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct XapkSplitFile {
    file: String,
    id: String,
}

/// A materialized container: the base APK and its splits extracted into a
/// scratch directory under their container entry names. Dropping the bundle
/// deletes the extracted files.
#[derive(Debug)]
pub struct ContainerBundle {
    format: ContainerFormat,
    package: String,
    base_entry: String,
    split_entries: Vec<String>,
    base_path: PathBuf,
    split_paths: Vec<PathBuf>,
    _scratch: ScratchDir,
}

impl ContainerBundle {
    /// Cheaply answers whether a path is a supported container, without
    /// extracting anything: by the container metadata when recognizable,
    /// else by the file extension. An APK's own manifest takes precedence.
    pub fn sniff(path: &Path) -> Result<Option<ContainerFormat>> {
        let mut archive = reader::open_archive(path)?;
        detect_format(&mut archive, path)
    }

    /// Opens and materializes a container; `None` when `path` is not one.
    #[instrument(level = "info", skip_all, fields(path = %path.display()))]
    pub fn open(path: &Path) -> Result<Option<Self>> {
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
        let (base_entry, split_entries, package) =
            classify(&mut archive, metadata.as_ref(), &apk_entries, &scratch)?;
        let base_path = scratch.path().join(&base_entry);
        let split_paths = split_entries
            .iter()
            .map(|name| scratch.path().join(name))
            .collect();
        Ok(Some(Self {
            format,
            package,
            base_entry,
            split_entries,
            base_path,
            split_paths,
            _scratch: scratch,
        }))
    }

    pub fn format(&self) -> ContainerFormat {
        self.format
    }

    /// Package name from the validated base manifest.
    pub fn package(&self) -> &str {
        &self.package
    }

    /// The container entry naming the base APK, also its extracted file name.
    pub fn base_entry(&self) -> &str {
        &self.base_entry
    }

    pub fn split_entries(&self) -> &[String] {
        &self.split_entries
    }

    /// The extracted base APK.
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    /// The extracted split APKs, sorted by entry name.
    pub fn split_paths(&self) -> &[PathBuf] {
        &self.split_paths
    }
}

fn detect_format(archive: &mut Archive, path: &Path) -> Result<Option<ContainerFormat>> {
    // APKs can themselves contain JSON metadata and embedded APK assets.
    if reader::contains(archive, MANIFEST_ENTRY) {
        return Ok(None);
    }
    for (entry, marker, format) in [
        (INFO_JSON, "apkm_version", ContainerFormat::Apkm),
        (MANIFEST_JSON, "xapk_version", ContainerFormat::Xapk),
    ] {
        if reader::contains(archive, entry) {
            let data = reader::read_entry(archive, entry)?;
            if serde_json::from_slice::<serde_json::Value>(&data)
                .ok()
                .is_some_and(|info| info.get(marker).is_some_and(serde_json::Value::is_u64))
            {
                return Ok(Some(format));
            }
        }
    }
    Ok(extension_format(path))
}

fn read_metadata(archive: &mut Archive, name: &str) -> Result<Option<ContainerMetadata>> {
    if !reader::contains(archive, name) {
        return Ok(None);
    }
    let data = reader::read_entry(archive, name)?;
    serde_json::from_slice(&data)
        .map(Some)
        .map_err(|error| invalid("container", format!("malformed {name}: {error}")))
}

/// Every APK is classified by its own manifest. Container metadata can
/// confirm that classification, but cannot override it.
fn classify(
    archive: &mut Archive,
    metadata: Option<&ContainerMetadata>,
    entries: &[String],
    scratch: &ScratchDir,
) -> Result<(String, Vec<String>, String)> {
    let mut manifests = HashMap::new();
    let mut base = None;
    let mut splits = Vec::new();
    let mut split_names = HashSet::new();
    for entry in entries {
        let path = extract_entry(archive, entry, scratch.path())?;
        let manifest = manifest_of(path, entry)?;
        if let Some(name) = manifest.split_name() {
            if name.is_empty() || !split_names.insert(name.into_owned()) {
                return Err(invalid(
                    "container",
                    format!("empty or duplicate split name in {entry}"),
                ));
            }
            splits.push(entry.clone());
        } else if let Some(previous) = &base {
            return Err(invalid(
                "container",
                format!("multiple base APKs: {previous} and {entry}"),
            ));
        } else {
            base = Some(entry.clone());
        }
        manifests.insert(entry.as_str(), manifest);
    }
    let base = base.ok_or_else(|| {
        invalid(
            "container",
            "no base APK: no entry carries a base AndroidManifest.xml",
        )
    })?;
    let base_manifest = &manifests[base.as_str()];
    let package = base_manifest
        .package_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| invalid("container", "base APK has no package name"))?
        .into_owned();
    for entry in &splits {
        let manifest = &manifests[entry.as_str()];
        if manifest.package_name().as_deref() != Some(package.as_str())
            || manifest.version_code() != base_manifest.version_code()
        {
            return Err(invalid(
                "container",
                format!("{entry} does not match the base APK package and version code"),
            ));
        }
    }
    if let Some(metadata) = metadata {
        if metadata
            .package_name
            .as_deref()
            .is_some_and(|name| name != package)
        {
            return Err(invalid(
                "container",
                "metadata package does not match the base APK",
            ));
        }
        if let Some(mapping) = &metadata.split_apks {
            let mut listed = HashSet::new();
            for split in mapping {
                let manifest = manifests.get(split.file.as_str()).ok_or_else(|| {
                    invalid(
                        "container",
                        format!("manifest references missing APK {}", split.file),
                    )
                })?;
                if !listed.insert(split.file.as_str()) {
                    return Err(invalid(
                        "container",
                        format!("duplicate APK reference {}", split.file),
                    ));
                }
                let name = manifest.split_name();
                if split.id != name.as_deref().unwrap_or("base") {
                    return Err(invalid(
                        "container",
                        format!(
                            "metadata split id for {} does not match its APK manifest",
                            split.file
                        ),
                    ));
                }
            }
            if listed.len() != entries.len() {
                return Err(invalid(
                    "container",
                    "unlisted APK entries in container metadata",
                ));
            }
        }
    }
    Ok((base, splits, package))
}

fn apk_entries(archive: &Archive) -> Result<Vec<String>> {
    let mut entries = Vec::new();
    for name in archive.file_names() {
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".obb") {
            return Err(invalid(
                "container",
                "XAPK expansion files are not supported",
            ));
        }
        if lower.ends_with(".apk") {
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

/// Streams the entry into `dir` under its name and returns the new path,
/// checking the ZIP CRC as the entry is read to completion.
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

/// Parses the `AndroidManifest.xml` inside an extracted APK file.
fn manifest_of(path: PathBuf, entry: &str) -> Result<AxmlDocument> {
    let mut archive = reader::open_archive(&path)?;
    let data = reader::read_entry(&mut archive, MANIFEST_ENTRY)?;
    AxmlDocument::parse(&data)
        .map_err(|error| invalid("container", format!("{entry} is not a valid APK: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_format_is_case_insensitive() {
        assert_eq!(
            extension_format(Path::new("App.APKM")),
            Some(ContainerFormat::Apkm)
        );
        assert_eq!(
            extension_format(Path::new("App.xapk")),
            Some(ContainerFormat::Xapk)
        );
        assert_eq!(extension_format(Path::new("App.apk")), None);
    }
}
