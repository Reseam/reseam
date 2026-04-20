// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::axml::reader::AxmlDocument;
use crate::dex;
use crate::error::{invalid, Result};
use crate::resources::ResourceTable;
use crate::zip::reader::ApkReader;
use crate::zip::writer::ApkWriter;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use reseam_dex::{MultiDexContainer, ParseOptions};
use tracing::{debug, info, instrument};

/// The kind of APK: single or split bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApkKind {
    Single,
    Split,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ApkWriteOptions {
    pub strip_signatures: bool,
}

/// Metadata for a single APK component (base or split).
#[derive(Debug, Clone)]
pub struct ApkComponent {
    /// Human-readable name (e.g. "base", "split_config.arm64_v8a").
    pub name: String,
    /// Original file path.
    pub path: PathBuf,
    /// DEX entry names originally present in this APK.
    pub original_dex_names: Vec<String>,
}

#[derive(Debug, Clone)]
struct ApkComponentState {
    meta: ApkComponent,
    manifest: AxmlDocument,
    manifest_dirty: bool,
    resources: Option<ResourceTable>,
    resources_dirty: bool,
    entry_names: Vec<String>,
    injected_files: HashMap<String, (Vec<u8>, zip::CompressionMethod)>,
    deleted_files: HashSet<String>,
}

impl ApkComponentState {
    fn read_entry(&self, entry_name: &str) -> Result<Option<Vec<u8>>> {
        if self.deleted_files.contains(entry_name) {
            return Ok(None);
        }
        if let Some((data, _)) = self.injected_files.get(entry_name) {
            return Ok(Some(data.clone()));
        }
        if entry_name == "AndroidManifest.xml" && self.manifest_dirty {
            return self.manifest.serialize().map(Some);
        }
        if entry_name == "resources.arsc" && self.resources_dirty {
            if let Some(resources) = &self.resources {
                return resources.serialize().map(Some);
            }
        }

        let file = File::open(&self.meta.path)?;
        let mut reader = ApkReader::new(BufReader::new(file))?;
        if reader.contains(entry_name) {
            return reader.read_entry(entry_name).map(Some);
        }
        Ok(None)
    }

    fn inject_file(&mut self, path: &str, data: Vec<u8>, compression: zip::CompressionMethod) {
        self.injected_files
            .insert(path.to_string(), (data, compression));
        self.deleted_files.remove(path);
        if !self.entry_names.iter().any(|entry| entry == path) {
            self.entry_names.push(path.to_string());
        }
    }

    fn delete_file(&mut self, path: &str) {
        self.deleted_files.insert(path.to_string());
        self.injected_files.remove(path);
        self.entry_names.retain(|entry| entry != path);
    }
}

/// An opened APK file with parsed DEX and manifest access.
///
/// Supports both single APKs and split APK bundles. For split bundles,
/// DEX from all APKs is unified into a single `MultiDexContainer`, while
/// manifest/resource state remains tracked per component.
pub struct ApkFile {
    kind: ApkKind,
    dex: MultiDexContainer,
    dex_dirty: bool,
    components: Vec<ApkComponentState>,
    entry_names: Vec<String>,
}

impl ApkFile {
    /// Open a single APK from a file path.
    #[instrument(level = "info", skip_all, fields(apk_path = %path.as_ref().display()))]
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_options(path, ParseOptions::default())
    }

    /// Open a single APK with custom parse options.
    #[instrument(level = "info", skip_all, fields(apk_path = %path.as_ref().display(), lazy = opts.lazy))]
    pub fn open_with_options(path: impl AsRef<Path>, opts: ParseOptions) -> Result<Self> {
        let path = path.as_ref();
        let component = Self::load_component(path, "base".to_string())?;
        let dex = Self::load_component_dex(&component.meta.path, opts)?;

        info!(
            package = component.manifest.package_name(),
            version = component.manifest.version_name(),
            dex_files = dex.len(),
            has_resources = component.resources.is_some(),
            "opened APK"
        );

        Ok(Self {
            kind: ApkKind::Single,
            dex,
            dex_dirty: false,
            entry_names: component.entry_names.clone(),
            components: vec![component],
        })
    }

    /// Open a split APK bundle (base + splits).
    #[instrument(level = "info", skip_all, fields(base_path = %base.as_ref().display(), split_count = splits.len()))]
    pub fn open_split(base: impl AsRef<Path>, splits: &[impl AsRef<Path>]) -> Result<Self> {
        Self::open_split_with_options(base, splits, ParseOptions::default())
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
        let base_component = Self::load_component(base_path, "base".to_string())?;
        let mut entry_names = base_component.entry_names.clone();
        components.push(base_component);

        let mut all_buffers: Vec<Vec<u8>> = Vec::new();
        Self::push_component_dex(&mut all_buffers, base_path)?;

        for split_path in splits {
            let split_path = split_path.as_ref();
            let split_name = Self::derive_split_name(split_path);
            let component = Self::load_component(split_path, split_name)?;
            entry_names.extend(component.entry_names.iter().cloned());
            Self::push_component_dex(&mut all_buffers, split_path)?;
            components.push(component);
        }

        let refs: Vec<&[u8]> = all_buffers.iter().map(|b| b.as_slice()).collect();
        let dex = if refs.is_empty() {
            MultiDexContainer::new()
        } else {
            MultiDexContainer::parse(&refs, opts)?
        };

        let entry_names = dedupe_preserve_order(entry_names);

        info!(
            component_count = components.len(),
            dex_files = dex.len(),
            "opened split APK bundle"
        );

        Ok(Self {
            kind: ApkKind::Split,
            dex,
            dex_dirty: false,
            components,
            entry_names,
        })
    }

    fn load_component(path: &Path, name: String) -> Result<ApkComponentState> {
        let file = File::open(path)?;
        let mut reader = ApkReader::new(BufReader::new(file))?;
        let manifest_bytes = reader.read_manifest()?;
        let manifest = AxmlDocument::parse(&manifest_bytes)?;
        let resources = if reader.contains("resources.arsc") {
            let arsc_bytes = reader.read_entry("resources.arsc")?;
            Some(ResourceTable::parse(&arsc_bytes)?)
        } else {
            None
        };
        let entry_names = reader.entry_names();
        let original_dex_names = reader.dex_entry_names();

        Ok(ApkComponentState {
            meta: ApkComponent {
                name,
                path: path.to_path_buf(),
                original_dex_names,
            },
            manifest,
            manifest_dirty: false,
            resources,
            resources_dirty: false,
            entry_names,
            injected_files: HashMap::new(),
            deleted_files: HashSet::new(),
        })
    }

    fn load_component_dex(path: &Path, opts: ParseOptions) -> Result<MultiDexContainer> {
        let file = File::open(path)?;
        let mut reader = ApkReader::new(BufReader::new(file))?;
        dex::extract_dex(&mut reader, opts)
    }

    fn push_component_dex(out: &mut Vec<Vec<u8>>, path: &Path) -> Result<()> {
        let file = File::open(path)?;
        let mut reader = ApkReader::new(BufReader::new(file))?;
        for (_, buf) in reader.read_all_dex()? {
            out.push(buf);
        }
        Ok(())
    }

    fn derive_split_name(path: &Path) -> String {
        let manifest_bytes = File::open(path)
            .ok()
            .and_then(|file| ApkReader::new(BufReader::new(file)).ok())
            .and_then(|mut reader| reader.read_manifest().ok());

        manifest_bytes
            .and_then(|b| AxmlDocument::parse(&b).ok())
            .and_then(|doc| doc.split_name().map(|s| s.to_string()))
            .unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string()
            })
    }

    fn base_component(&self) -> &ApkComponentState {
        &self.components[0]
    }

    fn base_component_mut(&mut self) -> &mut ApkComponentState {
        &mut self.components[0]
    }

    fn component(&self, index: usize) -> Option<&ApkComponentState> {
        self.components.get(index)
    }

    fn component_mut(&mut self, index: usize) -> Option<&mut ApkComponentState> {
        self.components.get_mut(index)
    }

    fn touch_entry_name(&mut self, path: &str) {
        if !self.entry_names.iter().any(|entry| entry == path) {
            self.entry_names.push(path.to_string());
        }
    }

    fn remove_entry_name_if_absent(&mut self, path: &str) {
        let exists = self
            .components
            .iter()
            .any(|component| component.entry_names.iter().any(|entry| entry == path));
        if !exists {
            self.entry_names.retain(|entry| entry != path);
        }
    }

    /// Get a reference to the unified DEX container.
    pub fn dex(&self) -> &MultiDexContainer {
        &self.dex
    }

    /// Get a mutable reference to the unified DEX container.
    pub fn dex_mut(&mut self) -> &mut MultiDexContainer {
        self.dex_dirty = true;
        &mut self.dex
    }

    pub fn manifest(&self) -> &AxmlDocument {
        &self.base_component().manifest
    }

    pub fn manifest_mut(&mut self) -> &mut AxmlDocument {
        let component = self.base_component_mut();
        component.manifest_dirty = true;
        &mut component.manifest
    }

    pub fn resources(&self) -> Option<&ResourceTable> {
        self.base_component().resources.as_ref()
    }

    pub fn resources_mut(&mut self) -> Option<&mut ResourceTable> {
        let component = self.base_component_mut();
        component.resources_dirty = true;
        component.resources.as_mut()
    }

    pub fn component_meta(&self, index: usize) -> Option<&ApkComponent> {
        self.component(index).map(|component| &component.meta)
    }

    pub fn component_manifest(&self, index: usize) -> Option<&AxmlDocument> {
        self.component(index).map(|component| &component.manifest)
    }

    pub fn component_manifest_mut(&mut self, index: usize) -> Option<&mut AxmlDocument> {
        let component = self.component_mut(index)?;
        component.manifest_dirty = true;
        Some(&mut component.manifest)
    }

    pub fn component_resources(&self, index: usize) -> Option<&ResourceTable> {
        self.component(index).and_then(|component| component.resources.as_ref())
    }

    pub fn component_resources_mut(&mut self, index: usize) -> Option<&mut ResourceTable> {
        let component = self.component_mut(index)?;
        component.resources_dirty = true;
        component.resources.as_mut()
    }

    pub fn component_entry_names(&self, index: usize) -> Option<&[String]> {
        self.component(index).map(|component| component.entry_names.as_slice())
    }

    pub fn component_index_by_name(&self, name: &str) -> Option<usize> {
        self.components
            .iter()
            .position(|component| component.meta.name == name)
    }

    pub fn find_resource_component(&self, type_name: &str, entry_name: &str) -> Option<usize> {
        self.components.iter().enumerate().find_map(|(index, component)| {
            component
                .resources
                .as_ref()
                .and_then(|resources| resources.find_resource_id(type_name, entry_name))
                .map(|_| index)
        })
    }

    pub fn find_resource_component_by_id(&self, res_id: u32) -> Option<usize> {
        self.components.iter().enumerate().find_map(|(index, component)| {
            component
                .resources
                .as_ref()
                .filter(|resources| resources.contains_resource_id(res_id))
                .map(|_| index)
        })
    }

    pub fn find_resource_id(&self, type_name: &str, entry_name: &str) -> Option<u32> {
        self.components.iter().find_map(|component| {
            component
                .resources
                .as_ref()
                .and_then(|resources| resources.find_resource_id(type_name, entry_name))
        })
    }

    pub fn resource_exists(&self, type_name: &str, entry_name: &str) -> bool {
        self.find_resource_component(type_name, entry_name).is_some()
    }

    pub fn get_string_resource_value(&self, name: &str) -> Option<&str> {
        self.components.iter().find_map(|component| {
            component
                .resources
                .as_ref()
                .and_then(|resources| resources.get_string_value(name))
        })
    }

    pub fn set_string_resource_value(&mut self, name: &str, value: &str) -> bool {
        let component_index = self.find_resource_component("string", name).unwrap_or(0);
        let Some(component) = self.component_mut(component_index) else {
            return false;
        };
        let Some(resources) = component.resources.as_mut() else {
            return false;
        };
        component.resources_dirty = true;
        resources.set_string_value(name, value)
    }

    pub fn component_find_resource_id(
        &self,
        component_index: usize,
        type_name: &str,
        entry_name: &str,
    ) -> Option<u32> {
        self.component(component_index)
            .and_then(|component| component.resources.as_ref())
            .and_then(|resources| resources.find_resource_id(type_name, entry_name))
    }

    /// Get the package name from the base manifest.
    pub fn package_name(&self) -> Option<&str> {
        self.base_component().manifest.package_name()
    }

    /// Get the version code from the base manifest.
    pub fn version_code(&self) -> Option<u32> {
        self.base_component().manifest.version_code()
    }

    /// Get the version name from the base manifest.
    pub fn version_name(&self) -> Option<&str> {
        self.base_component().manifest.version_name()
    }

    /// Whether this is a split APK bundle.
    pub fn is_split(&self) -> bool {
        self.kind == ApkKind::Split
    }

    /// Number of APK components (1 for single, 1+N for splits).
    pub fn component_count(&self) -> usize {
        self.components.len()
    }

    /// Get the split names (empty for single APKs, excludes base).
    pub fn split_names(&self) -> Vec<&str> {
        self.components
            .iter()
            .skip(1)
            .map(|c| c.meta.name.as_str())
            .collect()
    }

    pub fn entry_names(&self) -> &[String] {
        &self.entry_names
    }

    pub fn read_entry(&self, entry_name: &str) -> Result<Vec<u8>> {
        for component in &self.components {
            if let Some(data) = component.read_entry(entry_name)? {
                return Ok(data);
            }
        }
        Err(crate::error::ApkError::Invalid {
            section: "apk entry",
            reason: format!("entry not found in any component: {entry_name}"),
        })
    }

    pub fn read_entry_from_component(&self, index: usize, entry_name: &str) -> Result<Vec<u8>> {
        let component = self.component(index).ok_or_else(|| {
            invalid("apk component", format!("component index out of range: {index}"))
        })?;
        component.read_entry(entry_name)?.ok_or_else(|| {
            invalid(
                "apk entry",
                format!(
                    "entry not found in component {}: {entry_name}",
                    component.meta.name
                ),
            )
        })
    }

    /// Inject a file into the base component.
    pub fn inject_file(&mut self, path: &str, data: Vec<u8>) {
        self.inject_file_into(0, path, data);
    }

    /// Inject a stored file into the base component.
    pub fn inject_file_stored(&mut self, path: &str, data: Vec<u8>) {
        self.inject_file_stored_into(0, path, data);
    }

    pub fn inject_file_into(&mut self, component_index: usize, path: &str, data: Vec<u8>) {
        self.inject_file_with_method(component_index, path, data, zip::CompressionMethod::Deflated);
    }

    pub fn inject_file_stored_into(
        &mut self,
        component_index: usize,
        path: &str,
        data: Vec<u8>,
    ) {
        self.inject_file_with_method(component_index, path, data, zip::CompressionMethod::Stored);
    }

    fn inject_file_with_method(
        &mut self,
        component_index: usize,
        path: &str,
        data: Vec<u8>,
        compression: zip::CompressionMethod,
    ) {
        if let Some(component) = self.component_mut(component_index) {
            component.inject_file(path, data, compression);
            self.touch_entry_name(path);
        }
    }

    /// Delete a file from the base component.
    pub fn delete_file(&mut self, path: &str) {
        self.delete_file_from(0, path);
    }

    pub fn delete_file_from(&mut self, component_index: usize, path: &str) {
        if let Some(component) = self.component_mut(component_index) {
            component.delete_file(path);
            self.remove_entry_name_if_absent(path);
        }
    }

    /// Write the (possibly modified) APK to an output directory.
    ///
    /// For single APKs: writes one file at `output_dir/<original_name>`.
    /// For split bundles: writes base + all splits into `output_dir/`.
    ///
    /// All DEX goes into the base APK. Split APKs have their DEX entries removed.
    #[instrument(level = "info", skip_all, fields(output_dir = %output_dir.as_ref().display(), component_count = self.components.len()))]
    pub fn write_to(&mut self, output_dir: impl AsRef<Path>) -> Result<()> {
        self.write_to_with_options(output_dir, ApkWriteOptions::default())
    }

    #[instrument(level = "info", skip_all, fields(output_dir = %output_dir.as_ref().display(), component_count = self.components.len(), strip_signatures = options.strip_signatures))]
    pub fn write_to_with_options(
        &mut self,
        output_dir: impl AsRef<Path>,
        options: ApkWriteOptions,
    ) -> Result<()> {
        let output_dir = output_dir.as_ref();
        std::fs::create_dir_all(output_dir)?;

        let dex_entries = dex::dex_to_entries(&mut self.dex)?;
        self.dex_dirty = false;

        info!(
            dex_entry_count = dex_entries.len(),
            strip_signatures = options.strip_signatures,
            "serializing APK output"
        );

        for (idx, component) in self.components.iter_mut().enumerate() {
            let is_base = idx == 0;
            let output_path = output_dir.join(
                component
                    .meta
                    .path
                    .file_name()
                    .unwrap_or_else(|| std::ffi::OsStr::new("output.apk")),
            );

            Self::write_component(
                component,
                is_base,
                &dex_entries,
                &output_path,
                options.strip_signatures,
            )?;
            component.manifest_dirty = false;
            component.resources_dirty = false;
        }

        info!("APK write completed");
        Ok(())
    }

    fn write_component(
        component: &ApkComponentState,
        is_base: bool,
        dex_entries: &[(String, Vec<u8>)],
        output_path: &Path,
        strip_signatures: bool,
    ) -> Result<()> {
        debug!(
            component = %component.meta.name,
            is_base,
            output_path = %output_path.display(),
            "writing APK component"
        );
        let src_file = File::open(&component.meta.path)?;
        let src_reader = BufReader::new(src_file);
        let mut source = zip::ZipArchive::new(src_reader)?;

        let output_file = File::create(output_path)?;
        let mut writer = ApkWriter::new(output_file);

        let mut replacements: BTreeMap<String, (Vec<u8>, zip::CompressionMethod)> = BTreeMap::new();
        let mut removals: HashSet<String> = HashSet::new();

        for name in &component.meta.original_dex_names {
            removals.insert(name.clone());
        }

        if strip_signatures {
            for i in 0..source.len() {
                let name = {
                    let entry = source.by_index_raw(i)?;
                    entry.name().to_string()
                };
                if is_signature_entry(&name) {
                    removals.insert(name);
                }
            }
        }

        if is_base {
            for (name, data) in dex_entries {
                replacements.insert(
                    name.clone(),
                    (data.clone(), zip::CompressionMethod::Deflated),
                );
            }
        }

        if component.manifest_dirty {
            let manifest_bytes = component.manifest.serialize()?;
            replacements.insert(
                "AndroidManifest.xml".to_string(),
                (manifest_bytes, zip::CompressionMethod::Deflated),
            );
        }

        if component.resources_dirty {
            if let Some(resources) = &component.resources {
                let arsc_bytes = resources.serialize()?;
                replacements.insert(
                    "resources.arsc".to_string(),
                    (arsc_bytes, zip::CompressionMethod::Stored),
                );
            }
        }

        for (name, (data, method)) in &component.injected_files {
            replacements.insert(name.clone(), (data.clone(), *method));
        }
        for name in &component.deleted_files {
            removals.insert(name.clone());
        }

        writer.rewrite_apk(&mut source, &replacements, &removals)?;
        writer.finish()?;
        Ok(())
    }
}

fn is_signature_entry(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    upper == "META-INF/MANIFEST.MF"
        || upper.ends_with(".SF")
        || upper.ends_with(".RSA")
        || upper.ends_with(".DSA")
        || upper.ends_with(".EC")
}

fn dedupe_preserve_order(entries: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for entry in entries {
        if seen.insert(entry.clone()) {
            out.push(entry);
        }
    }
    out
}
