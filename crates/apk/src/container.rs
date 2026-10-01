// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashSet;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use serde::{Deserialize, de::IgnoredAny};
use tracing::instrument;

use crate::apk_file::validate_components;
use crate::entry::MANIFEST_ENTRY;
use crate::error::{Result, invalid};
use crate::zip::reader::{self, Archive};
use crate::{ApkComponent, ApkFile};
use reseam_dex::{ParseOptions, types::header::Loading};
use reseam_storage::ScratchDir;

const INFO_JSON: &str = "info.json";
const MANIFEST_JSON: &str = "manifest.json";

pub use reseam_model::ContainerFormat;

#[derive(Debug, Deserialize)]
struct ContainerMetadata {
    #[serde(alias = "pname")]
    package_name: Option<String>,
    split_apks: Option<Vec<XapkSplitFile>>,
    #[serde(default)]
    expansions: Vec<IgnoredAny>,
}

#[derive(Deserialize)]
struct ContainerMarker {
    apkm_version: Option<u64>,
    xapk_version: Option<u64>,
}

impl ContainerMarker {
    fn version(&self, format: ContainerFormat) -> Option<u64> {
        match format {
            ContainerFormat::Apkm => self.apkm_version,
            ContainerFormat::Xapk => self.xapk_version,
        }
    }
}

#[derive(Debug, Deserialize)]
struct XapkSplitFile {
    file: String,
    id: String,
}

/// A materialized container: the base APK and its splits extracted into a
/// scratch directory under their container entry names. Dropping the bundle
/// deletes the extracted files.
pub struct ContainerBundle {
    format: ContainerFormat,
    package: String,
    base_entry: String,
    split_entries: Vec<String>,
    base_path: PathBuf,
    split_paths: Vec<PathBuf>,
    components: Vec<ApkComponent>,
    scratch: ScratchDir,
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
        let components = classify(&archive, metadata.as_ref(), &apk_entries, &scratch)?;
        let base_entry = components[0]
            .path()
            .file_name()
            .expect("extracted filename")
            .to_string_lossy()
            .into_owned();
        let split_entries: Vec<_> = components[1..]
            .iter()
            .map(|component| {
                component
                    .path()
                    .file_name()
                    .expect("extracted filename")
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        let package = components[0]
            .manifest()
            .package_name()
            .expect("validated package")
            .into_owned();
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
            components,
            scratch,
        }))
    }

    /// Consumes the bundle into a session using its already parsed manifests.
    /// The session takes ownership of the extraction directory, so its paths
    /// remain valid until the session is dropped.
    pub fn into_apk(self, options: ParseOptions) -> Result<ApkFile> {
        let mut apk = ApkFile::from_components(self.components, options)?;
        apk.scratch = Some(self.scratch);
        Ok(apk)
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

fn read_metadata(archive: &mut Archive, name: &str) -> Result<Option<ContainerMetadata>> {
    if !reader::contains(archive, name) {
        return Ok(None);
    }
    let data = reader::read_entry(archive, name)?;
    serde_json::from_slice(&data)
        .map(Some)
        .map_err(|error| invalid("container", format!("malformed {name}: {error}")))
}

fn classify(
    archive: &Archive,
    metadata: Option<&ContainerMetadata>,
    entries: &[String],
    scratch: &ScratchDir,
) -> Result<Vec<ApkComponent>> {
    let mut components: Vec<_> = entries
        .par_iter()
        .map_with(archive.clone(), |archive, entry| {
            let path = extract_entry(archive, entry, scratch.path())?;
            ApkComponent::open(&path, Loading::Deferred)
        })
        .collect::<Result<_>>()?;
    components.sort_by_key(|component| component.manifest().split_name().is_some());
    validate_components(&components)?;
    let package = components[0]
        .manifest()
        .package_name()
        .expect("validated package");
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
                let component = components
                    .iter()
                    .find(|component| {
                        component
                            .path()
                            .file_name()
                            .is_some_and(|name| name == split.file.as_str())
                    })
                    .ok_or_else(|| {
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
                if split.id
                    != component
                        .manifest()
                        .split_name()
                        .as_deref()
                        .unwrap_or("base")
                {
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
    Ok(components)
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

impl std::fmt::Debug for ContainerBundle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContainerBundle")
            .field("format", &self.format)
            .field("base", &self.base_path)
            .field("splits", &self.split_paths)
            .finish_non_exhaustive()
    }
}
