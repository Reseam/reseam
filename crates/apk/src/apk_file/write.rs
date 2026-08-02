// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use tracing::{debug, info, instrument};

use super::{
    ApkComponentSession, ApkEntryPath, ApkFile, ApkWriteOptions, DexEntryOrigin, DexSessionEntry,
};
use crate::dex;
use crate::error::{invalid, Result};
use crate::zip::writer::{ApkReplacement, ApkWriter};

struct DexReplacementOwned {
    name: ApkEntryPath,
    data: Vec<u8>,
}

enum DexWritePlan {
    Passthrough,
    Mixed {
        by_component: Vec<Vec<DexReplacementOwned>>,
        origins_after_write: Vec<DexEntryOrigin>,
    },
    FullSerialize {
        base_entries: Vec<DexReplacementOwned>,
        origins_after_write: Vec<DexEntryOrigin>,
    },
}

impl DexWritePlan {
    fn mode(&self) -> &'static str {
        match self {
            Self::Passthrough => "passthrough",
            Self::Mixed { .. } => "partial",
            Self::FullSerialize { .. } => "full",
        }
    }

    fn replacement_count(&self) -> usize {
        match self {
            Self::Passthrough => 0,
            Self::Mixed { by_component, .. } => by_component.iter().map(Vec::len).sum(),
            Self::FullSerialize { base_entries, .. } => base_entries.len(),
        }
    }
}

impl ApkFile {
    /// Write the (possibly modified) APK to an output directory.
    ///
    /// For single APKs: writes one file at `output_dir/<original_name>`.
    /// For split bundles: writes base + all splits into `output_dir/`.
    ///
    /// Unchanged DEX and resources are copied through from the current source
    /// APKs. Modified DEX entries are rewritten back to their source component
    /// and name when possible; full base-only DEX serialization is only used
    /// when redistribution is required.
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

        let dex_plan = self.build_dex_write_plan()?;

        info!(
            dex_entry_count = self.dex.len(),
            dex_rewrite_count = dex_plan.replacement_count(),
            dex_write_mode = dex_plan.mode(),
            strip_signatures = options.strip_signatures,
            "serializing APK output"
        );

        let mut output_paths = Vec::with_capacity(self.components.len());
        for (idx, component) in self.components.iter().enumerate() {
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
                idx,
                is_base,
                &dex_plan,
                &output_path,
                options.strip_signatures,
            )?;
            output_paths.push(output_path);
        }

        self.commit_after_write(output_paths, dex_plan)?;

        info!("APK write completed");
        Ok(())
    }

    fn build_dex_write_plan(&mut self) -> Result<DexWritePlan> {
        if !self.any_dex_dirty() {
            return Ok(DexWritePlan::Passthrough);
        }

        if self.dex.needs_redistribute() {
            let dex_entries = dex::dex_to_entries(&mut self.dex)?;
            let origins_after_write = dex_entries
                .iter()
                .map(|(name, _)| DexEntryOrigin {
                    component_index: 0,
                    entry_name: name.clone().into(),
                })
                .collect();
            let base_entries = dex_entries
                .into_iter()
                .map(|(name, data)| DexReplacementOwned {
                    name: name.into(),
                    data,
                })
                .collect();
            return Ok(DexWritePlan::FullSerialize {
                base_entries,
                origins_after_write,
            });
        }

        let mut serialized = serialize_dirty_dexes(&mut self.dex, &self.dex_sessions)?;

        let mut by_component: Vec<Vec<DexReplacementOwned>> =
            (0..self.components.len()).map(|_| Vec::new()).collect();
        let mut origins_after_write = Vec::with_capacity(self.dex_sessions.len());
        let mut used_base_names: HashSet<String> = self
            .components
            .first()
            .map(|component| {
                component
                    .meta
                    .original_dex_names
                    .iter()
                    .map(|name| name.as_str().to_string())
                    .collect()
            })
            .unwrap_or_default();

        for index in 0..self.dex_sessions.len() {
            let session = &self.dex_sessions[index];
            match session.state {
                super::DexEntryState::Clean => {
                    let Some(origin) = session.origin.clone() else {
                        return Err(invalid(
                            "dex session",
                            format!("clean DEX entry {index} is missing an origin"),
                        ));
                    };
                    origins_after_write.push(origin);
                }
                super::DexEntryState::Modified => {
                    let Some(origin) = session.origin.clone() else {
                        return Err(invalid(
                            "dex session",
                            format!("modified DEX entry {index} is missing an origin"),
                        ));
                    };
                    by_component[origin.component_index].push(DexReplacementOwned {
                        name: origin.entry_name.clone(),
                        data: take_serialized(&mut serialized, index)?,
                    });
                    origins_after_write.push(origin);
                }
                super::DexEntryState::Added => {
                    let entry_name: ApkEntryPath =
                        next_free_base_dex_name(&mut used_base_names).into();
                    by_component[0].push(DexReplacementOwned {
                        name: entry_name.clone(),
                        data: take_serialized(&mut serialized, index)?,
                    });
                    origins_after_write.push(DexEntryOrigin {
                        component_index: 0,
                        entry_name,
                    });
                }
            }
        }

        Ok(DexWritePlan::Mixed {
            by_component,
            origins_after_write,
        })
    }

    fn commit_after_write(
        &mut self,
        output_paths: Vec<PathBuf>,
        dex_plan: DexWritePlan,
    ) -> Result<()> {
        for (component, output_path) in self.components.iter_mut().zip(output_paths) {
            component.finalize_write(output_path)?;
        }

        match dex_plan {
            DexWritePlan::Passthrough => {
                for session in &mut self.dex_sessions {
                    session.state = super::DexEntryState::Clean;
                }
            }
            DexWritePlan::Mixed {
                origins_after_write,
                ..
            }
            | DexWritePlan::FullSerialize {
                origins_after_write,
                ..
            } => {
                self.dex_sessions = origins_after_write
                    .into_iter()
                    .map(|origin| DexSessionEntry {
                        origin: Some(origin),
                        state: super::DexEntryState::Clean,
                    })
                    .collect();
            }
        }

        self.entry_names = dedupe_entry_names(
            self.components
                .iter()
                .flat_map(|component| component.entry_names.iter().cloned())
                .collect(),
        );
        Ok(())
    }

    fn write_component(
        component: &ApkComponentSession,
        component_index: usize,
        is_base: bool,
        dex_plan: &DexWritePlan,
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
                .resources_loaded()
                .map(|resources| resources.serialize())
                .transpose()?
        } else {
            None
        };

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

        match dex_plan {
            DexWritePlan::Passthrough => {}
            DexWritePlan::Mixed { by_component, .. } => {
                for replacement in &by_component[component_index] {
                    replacements.insert(
                        replacement.name.as_str(),
                        ApkReplacement {
                            data: replacement.data.as_slice(),
                            compression: zip::CompressionMethod::Deflated,
                        },
                    );
                }
            }
            DexWritePlan::FullSerialize { base_entries, .. } => {
                for name in &component.meta.original_dex_names {
                    removals.insert(name.to_string());
                }

                if is_base {
                    for replacement in base_entries {
                        replacements.insert(
                            replacement.name.as_str(),
                            ApkReplacement {
                                data: replacement.data.as_slice(),
                                compression: zip::CompressionMethod::Deflated,
                            },
                        );
                    }
                }
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

/// Serialize every non-clean DEX in parallel.
///
/// The result is positional: `result[i]` holds the serialized bytes for
/// `sessions[i]` when that entry is dirty, `None` when it is clean.
fn serialize_dirty_dexes(
    dex: &mut reseam_dex::MultiDexContainer,
    sessions: &[DexSessionEntry],
) -> Result<Vec<Option<Vec<u8>>>> {
    use rayon::prelude::*;

    dex.dex_files
        .par_iter_mut()
        .zip(sessions.par_iter())
        .map(|(dex, session)| match session.state {
            super::DexEntryState::Clean => Ok(None),
            super::DexEntryState::Modified | super::DexEntryState::Added => {
                Ok(Some(reseam_dex::write(dex)?))
            }
        })
        .collect()
}

fn take_serialized(serialized: &mut [Option<Vec<u8>>], index: usize) -> Result<Vec<u8>> {
    serialized
        .get_mut(index)
        .and_then(Option::take)
        .ok_or_else(|| {
            invalid(
                "dex session",
                format!("dirty DEX entry {index} has no serialized data"),
            )
        })
}

fn next_free_base_dex_name(used_names: &mut HashSet<String>) -> String {
    for index in 1u32.. {
        let candidate = if index == 1 {
            "classes.dex".to_string()
        } else {
            format!("classes{index}.dex")
        };
        if used_names.insert(candidate.clone()) {
            return candidate;
        }
    }
    unreachable!("DEX name search is unbounded");
}

fn dedupe_entry_names(entries: Vec<ApkEntryPath>) -> Vec<ApkEntryPath> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for entry in entries {
        if seen.insert(entry.clone()) {
            deduped.push(entry);
        }
    }
    deduped
}

fn is_signature_entry(name: &str) -> bool {
    let uppercase = name.to_ascii_uppercase();
    uppercase == "META-INF/MANIFEST.MF"
        || uppercase.ends_with(".SF")
        || uppercase.ends_with(".RSA")
        || uppercase.ends_with(".DSA")
        || uppercase.ends_with(".EC")
}
