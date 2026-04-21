// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use tracing::{debug, info, instrument};

use super::{ApkComponentSession, ApkFile, ApkWriteOptions};
use crate::dex;
use crate::zip::writer::ApkReplacement;
use crate::zip::writer::ApkWriter;
use crate::Result;

impl ApkFile {
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
        component: &ApkComponentSession,
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
        let source_file = File::open(&component.meta.path)?;
        let source_reader = BufReader::new(source_file);
        let mut source = zip::ZipArchive::new(source_reader)?;

        let output_file = File::create(output_path)?;
        let mut writer = ApkWriter::new(output_file);

        let mut replacements = BTreeMap::new();
        let mut removals = HashSet::new();
        let manifest_bytes = if component.manifest_dirty {
            Some(component.manifest.serialize()?)
        } else {
            None
        };
        let resource_bytes = if component.resources_dirty {
            component
                .resources
                .as_ref()
                .map(|resources| resources.serialize())
                .transpose()?
        } else {
            None
        };

        for name in &component.meta.original_dex_names {
            removals.insert(name.to_string());
        }

        if strip_signatures {
            for index in 0..source.len() {
                let name = {
                    let entry = source.by_index_raw(index)?;
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
                    name.as_str(),
                    ApkReplacement {
                        data: data.as_slice(),
                        compression: zip::CompressionMethod::Deflated,
                    },
                );
            }
        }

        if let Some(manifest_bytes) = manifest_bytes.as_deref() {
            replacements.insert(
                "AndroidManifest.xml",
                ApkReplacement {
                    data: manifest_bytes,
                    compression: zip::CompressionMethod::Deflated,
                },
            );
        }

        if let Some(resource_bytes) = resource_bytes.as_deref() {
            replacements.insert(
                "resources.arsc",
                ApkReplacement {
                    data: resource_bytes,
                    compression: zip::CompressionMethod::Stored,
                },
            );
        }

        for (name, (data, method)) in &component.injected_files {
            replacements.insert(
                name.as_str(),
                ApkReplacement {
                    data: data.as_slice(),
                    compression: *method,
                },
            );
        }
        for name in &component.deleted_files {
            removals.insert(name.to_string());
        }

        writer.rewrite_apk(&mut source, &replacements, &removals)?;
        writer.finish()?;
        Ok(())
    }
}

fn is_signature_entry(name: &str) -> bool {
    let uppercase = name.to_ascii_uppercase();
    uppercase == "META-INF/MANIFEST.MF"
        || uppercase.ends_with(".SF")
        || uppercase.ends_with(".RSA")
        || uppercase.ends_with(".DSA")
        || uppercase.ends_with(".EC")
}
