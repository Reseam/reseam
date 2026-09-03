// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

use reseam_dex::file::DexBytes;
use reseam_dex::{MultiDexContainer, ParseOptions};
use tracing::{info, instrument, warn};

use super::{
    ApkComponent, ApkComponentSession, ApkEntryPath, ApkFile, ApkKind, ComponentName,
    DexSessionEntry,
};
use crate::axml::reader::AxmlDocument;
use crate::dex;
use crate::resources::ResourceTable;
use crate::zip::reader::{entry_bytes, ApkReader};
use crate::Result;

impl ApkFile {
    /// Open a single APK from a file path.
    #[instrument(level = "info", skip_all, fields(apk_path = %path.as_ref().display()))]
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_options(path, ParseOptions::default())
    }

    /// Open a single APK for patching with lazy DEX class-data resolution enabled.
    #[instrument(level = "info", skip_all, fields(apk_path = %path.as_ref().display()))]
    pub fn open_for_patching(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_options(path, patch_parse_options())
    }

    /// Open a single APK with custom parse options.
    #[instrument(level = "info", skip_all, fields(apk_path = %path.as_ref().display(), lazy = opts.lazy))]
    pub fn open_with_options(path: impl AsRef<Path>, opts: ParseOptions) -> Result<Self> {
        let path = path.as_ref();
        let component = Self::load_component(path, "base".to_string(), &opts)?;
        let (dex, dex_names) = Self::load_component_dex(&component.meta.path, opts)?;
        let dex_sessions = dex_names
            .into_iter()
            .map(|name| DexSessionEntry::existing(0, name))
            .collect();

        info!(
            package = component.manifest.package_name(),
            version = component.manifest.version_name(),
            dex_files = dex.len(),
            has_resources = component.has_resource_entry(),
            "opened APK"
        );

        Ok(Self {
            kind: ApkKind::Single,
            dex,
            dex_sessions,
            entry_names: component.entry_names.clone(),
            components: vec![component],
        })
    }

    /// Open a split APK bundle (base + splits).
    #[instrument(level = "info", skip_all, fields(base_path = %base.as_ref().display(), split_count = splits.len()))]
    pub fn open_split(base: impl AsRef<Path>, splits: &[impl AsRef<Path>]) -> Result<Self> {
        Self::open_split_with_options(base, splits, ParseOptions::default())
    }

    /// Open a split APK set for patching with lazy DEX class-data resolution enabled.
    #[instrument(level = "info", skip_all, fields(base_path = %base.as_ref().display(), split_count = splits.len()))]
    pub fn open_split_for_patching(
        base: impl AsRef<Path>,
        splits: &[impl AsRef<Path>],
    ) -> Result<Self> {
        Self::open_split_with_options(base, splits, patch_parse_options())
    }

    /// Open a split APK bundle with custom parse options.
    #[instrument(level = "info", skip_all, fields(base_path = %base.as_ref().display(), split_count = splits.len(), lazy = opts.lazy))]
    pub fn open_split_with_options(
        base: impl AsRef<Path>,
        splits: &[impl AsRef<Path>],
        opts: ParseOptions,
    ) -> Result<Self> {
        let base_path = base.as_ref();
        let mut components = Vec::with_capacity(1 + splits.len());
        let base_component = Self::load_component(base_path, "base".to_string(), &opts)?;
        let mut entry_names = base_component.entry_names.clone();
        let (mut dex, base_dex_names) = Self::load_component_dex(base_path, opts.clone())?;
        let mut dex_sessions: Vec<DexSessionEntry> = base_dex_names
            .into_iter()
            .map(|name| DexSessionEntry::existing(0, name))
            .collect();
        components.push(base_component);

        for split_path in splits {
            let split_path = split_path.as_ref();
            let split_name = Self::derive_split_name(split_path);
            let component = Self::load_component(split_path, split_name, &opts)?;
            entry_names.extend(component.entry_names.iter().cloned());
            let component_index = components.len();
            let (split_dex, split_dex_names) = Self::load_component_dex(split_path, opts.clone())?;
            dex.extend(split_dex);
            dex_sessions.extend(
                split_dex_names
                    .into_iter()
                    .map(|name| DexSessionEntry::existing(component_index, name)),
            );
            components.push(component);
        }

        info!(
            component_count = components.len(),
            dex_files = dex.len(),
            "opened split APK bundle"
        );

        Ok(Self {
            kind: ApkKind::Split,
            dex,
            dex_sessions,
            components,
            entry_names: dedupe_preserve_order(entry_names),
        })
    }

    fn load_component(
        path: &Path,
        name: String,
        opts: &ParseOptions,
    ) -> Result<ApkComponentSession> {
        let file = File::open(path)?;
        let archive_file = file.try_clone()?;
        let mut reader = ApkReader::new(BufReader::new(file))?;
        let manifest_bytes = reader.read_manifest()?;
        let manifest = AxmlDocument::parse(&manifest_bytes)?;
        let resources = if reader.contains("resources.arsc") {
            if opts.lazy {
                super::component::ResourceSession::Deferred
            } else {
                let arsc_bytes = entry_bytes(&archive_file, &mut reader.archive_mut().by_name("resources.arsc")?)?;
                super::component::ResourceSession::Loaded(ResourceTable::parse(
                    DexBytes::from_mmap(Arc::new(arsc_bytes)),
                )?)
            }
        } else {
            super::component::ResourceSession::Absent
        };
        let entry_names = reader.entry_names();
        let original_dex_names = reader.dex_entry_names();

        Ok(ApkComponentSession {
            meta: ApkComponent {
                name: ComponentName::from(name),
                path: path.to_path_buf(),
                original_dex_names: original_dex_names
                    .into_iter()
                    .map(ApkEntryPath::from)
                    .collect(),
            },
            manifest,
            manifest_dirty: false,
            resources,
            resources_dirty: false,
            entry_names: entry_names.into_iter().map(ApkEntryPath::from).collect(),
            injected_files: HashMap::new(),
            deleted_files: HashSet::new(),
        })
    }

    fn load_component_dex(
        path: &Path,
        opts: ParseOptions,
    ) -> Result<(MultiDexContainer, Vec<ApkEntryPath>)> {
        let (dex, names) = dex::extract_dex(path, opts)?;
        Ok((dex, names.into_iter().map(ApkEntryPath::from).collect()))
    }

    fn derive_split_name(path: &Path) -> String {
        let manifest_bytes = File::open(path)
            .ok()
            .and_then(|file| ApkReader::new(BufReader::new(file)).ok())
            .and_then(|mut reader| reader.read_manifest().ok());

        manifest_bytes
            .and_then(|bytes| AxmlDocument::parse(&bytes).ok())
            .and_then(|document| document.split_name().map(|name| name.to_string()))
            .unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|name| name.to_str())
                    .unwrap_or("unknown")
                    .to_string()
            })
    }
}

fn patch_parse_options() -> ParseOptions {
    ParseOptions {
        lazy: true,
        include_debug_info: false,
        include_annotations: true,
        // The ZIP CRC already validated each entry during extraction;
        // re-hashing every DEX (Adler-32 + SHA-1) adds nothing here.
        skip_checksum: true,
        skip_signature: true,
        ..ParseOptions::default()
    }
}

fn dedupe_preserve_order(entries: Vec<ApkEntryPath>) -> Vec<ApkEntryPath> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for entry in entries {
        if seen.insert(entry.clone()) {
            deduped.push(entry);
        }
    }
    deduped
}
