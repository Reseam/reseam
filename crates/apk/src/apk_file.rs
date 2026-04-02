use crate::axml::reader::AxmlDocument;
use crate::dex;
use crate::error::Result;
use crate::resources::ResourceTable;
use crate::zip::reader::ApkReader;
use crate::zip::writer::ApkWriter;
use stitch_dex::{MultiDexContainer, ParseOptions};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use tracing::{debug, info, instrument};

/// The kind of APK: single or split bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApkKind {
    Single,
    Split,
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

/// An opened APK file with parsed DEX and manifest access.
///
/// Supports both single APKs and split APK bundles. For split bundles,
/// DEX from all APKs is unified into a single `MultiDexContainer`.
pub struct ApkFile {
    kind: ApkKind,
    dex: MultiDexContainer,
    manifest: AxmlDocument,
    manifest_dirty: bool,
    resources: Option<ResourceTable>,
    resources_dirty: bool,
    components: Vec<ApkComponent>,
    entry_names: Vec<String>,
    injected_files: HashMap<String, (Vec<u8>, zip::CompressionMethod)>,
    deleted_files: HashSet<String>,
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
        let dex_names = reader.dex_entry_names();
        let dex = dex::extract_dex(&mut reader, opts)?;

        let component = ApkComponent {
            name: "base".to_string(),
            path: path.to_path_buf(),
            original_dex_names: dex_names,
        };

        info!(
            package = manifest.package_name(),
            version = manifest.version_name(),
            dex_files = dex.len(),
            has_resources = resources.is_some(),
            "opened APK"
        );

        Ok(Self {
            kind: ApkKind::Single,
            dex,
            manifest,
            manifest_dirty: false,
            resources,
            resources_dirty: false,
            components: vec![component],
            entry_names,
            injected_files: HashMap::new(),
            deleted_files: HashSet::new(),
        })
    }

    /// Open a split APK bundle (base + splits).
    #[instrument(level = "info", skip_all, fields(base_path = %base.as_ref().display(), split_count = splits.len()))]
    pub fn open_split(
        base: impl AsRef<Path>,
        splits: &[impl AsRef<Path>],
    ) -> Result<Self> {
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

        // Open base APK
        let base_file = File::open(base_path)?;
        let mut base_reader = ApkReader::new(BufReader::new(base_file))?;

        let manifest_bytes = base_reader.read_manifest()?;
        let manifest = AxmlDocument::parse(&manifest_bytes)?;

        let resources = if base_reader.contains("resources.arsc") {
            let arsc_bytes = base_reader.read_entry("resources.arsc")?;
            Some(ResourceTable::parse(&arsc_bytes)?)
        } else {
            None
        };

        let base_dex_names = base_reader.dex_entry_names();
        let mut entry_names = base_reader.entry_names();

        // Collect all readers for unified DEX extraction
        let mut all_buffers: Vec<Vec<u8>> = Vec::new();
        let mut components = Vec::with_capacity(1 + splits.len());

        // Extract DEX from base
        let base_dex_entries = base_reader.read_all_dex()?;
        for (_, buf) in base_dex_entries {
            all_buffers.push(buf);
        }

        components.push(ApkComponent {
            name: "base".to_string(),
            path: base_path.to_path_buf(),
            original_dex_names: base_dex_names,
        });

        // Extract DEX from each split
        for split_path in splits {
            let split_path = split_path.as_ref();
            let split_file = File::open(split_path)?;
            let mut split_reader = ApkReader::new(BufReader::new(split_file))?;

            let split_dex_names = split_reader.dex_entry_names();
            entry_names.extend(split_reader.entry_names());
            let split_dex_entries = split_reader.read_all_dex()?;

            // Derive split name from manifest or filename
            let split_name = {
                let manifest_bytes = split_reader.read_manifest().ok();
                manifest_bytes
                    .and_then(|b| AxmlDocument::parse(&b).ok())
                    .and_then(|doc| doc.split_name().map(|s| s.to_string()))
                    .unwrap_or_else(|| {
                        split_path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown")
                            .to_string()
                    })
            };

            for (_, buf) in split_dex_entries {
                all_buffers.push(buf);
            }

            components.push(ApkComponent {
                name: split_name,
                path: split_path.to_path_buf(),
                original_dex_names: split_dex_names,
            });
        }

        // Parse all DEX into unified container
        let refs: Vec<&[u8]> = all_buffers.iter().map(|b| b.as_slice()).collect();
        let dex = if refs.is_empty() {
            MultiDexContainer::new()
        } else {
            MultiDexContainer::parse(&refs, opts)?
        };

        info!(
            component_count = components.len(),
            dex_files = dex.len(),
            has_resources = resources.is_some(),
            "opened split APK bundle"
        );

        Ok(Self {
            kind: ApkKind::Split,
            dex,
            manifest,
            manifest_dirty: false,
            resources,
            resources_dirty: false,
            components,
            entry_names,
            injected_files: HashMap::new(),
            deleted_files: HashSet::new(),
        })
    }

    /// Get a reference to the unified DEX container.
    pub fn dex(&self) -> &MultiDexContainer {
        &self.dex
    }

    /// Get a mutable reference to the unified DEX container.
    pub fn dex_mut(&mut self) -> &mut MultiDexContainer {
        &mut self.dex
    }

    pub fn manifest(&self) -> &AxmlDocument {
        &self.manifest
    }

    pub fn manifest_mut(&mut self) -> &mut AxmlDocument {
        self.manifest_dirty = true;
        &mut self.manifest
    }

    pub fn resources(&self) -> Option<&ResourceTable> {
        self.resources.as_ref()
    }

    pub fn resources_mut(&mut self) -> Option<&mut ResourceTable> {
        self.resources_dirty = true;
        self.resources.as_mut()
    }

    /// Get the package name from the manifest.
    pub fn package_name(&self) -> Option<&str> {
        self.manifest.package_name()
    }

    /// Get the version code from the manifest.
    pub fn version_code(&self) -> Option<u32> {
        self.manifest.version_code()
    }

    /// Get the version name from the manifest.
    pub fn version_name(&self) -> Option<&str> {
        self.manifest.version_name()
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
            .map(|c| c.name.as_str())
            .collect()
    }

    pub fn entry_names(&self) -> &[String] {
        &self.entry_names
    }

    pub fn read_entry(&self, entry_name: &str) -> Result<Vec<u8>> {
        if let Some((data, _)) = self.injected_files.get(entry_name) {
            return Ok(data.clone());
        }
        for component in &self.components {
            let file = File::open(&component.path)?;
            let mut reader = ApkReader::new(BufReader::new(file))?;
            if reader.contains(entry_name) {
                return reader.read_entry(entry_name);
            }
        }
        Err(crate::error::ApkError::Invalid {
            section: "apk entry",
            reason: format!("entry not found in any component: {entry_name}"),
        })
    }

    pub fn inject_file(&mut self, path: &str, data: Vec<u8>) {
        self.injected_files.insert(
            path.to_string(),
            (data, zip::CompressionMethod::Deflated),
        );
    }

    pub fn inject_file_stored(&mut self, path: &str, data: Vec<u8>) {
        self.injected_files
            .insert(path.to_string(), (data, zip::CompressionMethod::Stored));
    }

    pub fn delete_file(&mut self, path: &str) {
        self.deleted_files.insert(path.to_string());
    }

    /// Write the (possibly modified) APK to an output directory.
    ///
    /// For single APKs: writes one file at `output_dir/<original_name>`.
    /// For split bundles: writes base + all splits into `output_dir/`.
    ///
    /// All DEX goes into the base APK. Split APKs have their DEX entries removed.
    #[instrument(level = "info", skip_all, fields(output_dir = %output_dir.as_ref().display(), component_count = self.components.len()))]
    pub fn write_to(&mut self, output_dir: impl AsRef<Path>) -> Result<()> {
        let output_dir = output_dir.as_ref();
        std::fs::create_dir_all(output_dir)?;

        // Serialize all DEX from the unified container
        let dex_entries = dex::dex_to_entries(&mut self.dex)?;
        info!(dex_entry_count = dex_entries.len(), "serializing APK output");

        for (idx, component) in self.components.iter().enumerate() {
            let is_base = idx == 0;
            let output_path = output_dir.join(
                component
                    .path
                    .file_name()
                    .unwrap_or_else(|| std::ffi::OsStr::new("output.apk")),
            );

            self.write_component(component, is_base, &dex_entries, &output_path)?;
        }

        info!("APK write completed");
        Ok(())
    }

    fn write_component(
        &self,
        component: &ApkComponent,
        is_base: bool,
        dex_entries: &[(String, Vec<u8>)],
        output_path: &Path,
    ) -> Result<()> {
        debug!(
            component = %component.name,
            is_base,
            output_path = %output_path.display(),
            "writing APK component"
        );
        let src_file = File::open(&component.path)?;
        let src_reader = BufReader::new(src_file);
        let mut source = zip::ZipArchive::new(src_reader)?;

        let output_file = File::create(output_path)?;
        let mut writer = ApkWriter::new(output_file);

        if is_base {
            let mut replacements: BTreeMap<String, (Vec<u8>, zip::CompressionMethod)> =
                BTreeMap::new();
            let mut removals: HashSet<String> = HashSet::new();

            for name in &component.original_dex_names {
                removals.insert(name.clone());
            }

            for (name, data) in dex_entries {
                replacements.insert(
                    name.clone(),
                    (data.clone(), zip::CompressionMethod::Deflated),
                );
            }

            if self.manifest_dirty {
                let manifest_bytes = self.manifest.serialize()?;
                replacements.insert(
                    "AndroidManifest.xml".to_string(),
                    (manifest_bytes, zip::CompressionMethod::Deflated),
                );
            }

            if self.resources_dirty {
                if let Some(resources) = &self.resources {
                    let arsc_bytes = resources.serialize()?;
                    replacements.insert(
                        "resources.arsc".to_string(),
                        (arsc_bytes, zip::CompressionMethod::Stored),
                    );
                }
            }

            for (name, (data, method)) in &self.injected_files {
                replacements.insert(name.clone(), (data.clone(), *method));
            }
            for name in &self.deleted_files {
                removals.insert(name.clone());
            }

            writer.rewrite_apk(&mut source, &replacements, &removals)?;
        } else {
            let removals: HashSet<String> =
                component.original_dex_names.iter().cloned().collect();
            let replacements = BTreeMap::new();

            writer.rewrite_apk(&mut source, &replacements, &removals)?;
        }

        writer.finish()?;
        Ok(())
    }
}
