// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `info.json` (APKM) and `manifest.json` (XAPK): what a container says about
//! the APKs it holds.

use std::collections::HashSet;

use serde::{Deserialize, de::IgnoredAny};

use super::ContainerFormat;
use crate::ApkComponent;
use crate::error::{Result, invalid};
use crate::zip::reader::{self, Archive};

#[derive(Debug, Deserialize)]
pub(super) struct ContainerMetadata {
    #[serde(alias = "pname")]
    package_name: Option<String>,
    split_apks: Option<Vec<XapkSplitFile>>,
    #[serde(default)]
    pub(super) expansions: Vec<IgnoredAny>,
}

#[derive(Deserialize)]
pub(super) struct ContainerMarker {
    apkm_version: Option<u64>,
    xapk_version: Option<u64>,
}

impl ContainerMarker {
    pub(super) fn version(&self, format: ContainerFormat) -> Option<u64> {
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

pub(super) fn read_metadata(
    archive: &mut Archive,
    name: &str,
) -> Result<Option<ContainerMetadata>> {
    if !reader::contains(archive, name) {
        return Ok(None);
    }
    let data = reader::read_entry(archive, name)?;
    serde_json::from_slice(&data)
        .map(Some)
        .map_err(|error| invalid("container", format!("malformed {name}: {error}")))
}

/// Checks metadata against the APKs, whose manifests stay authoritative.
pub(super) fn check_metadata(
    metadata: &ContainerMetadata,
    components: &[ApkComponent],
    entry_count: usize,
) -> Result<()> {
    let package = components[0]
        .manifest()
        .package_name()
        .expect("ordered components have a base with a package");
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
    let Some(mapping) = &metadata.split_apks else {
        return Ok(());
    };
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
    if listed.len() != entry_count {
        return Err(invalid(
            "container",
            "unlisted APK entries in container metadata",
        ));
    }
    Ok(())
}
