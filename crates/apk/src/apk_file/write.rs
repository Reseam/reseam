// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use reseam_dex::DexFile;
use tracing::{debug, info, instrument};

use super::dex_workers::{DexEntryStream, DexWorkPool};
use super::{ApkComponent, ApkFile, DexOrigin};
use crate::entry::{
    dex_entry_name, is_signature_entry, next_free_dex_name, MANIFEST_ENTRY, RESOURCES_ENTRY,
};
use crate::error::Result;
use crate::zip::writer::{ApkWriter, Replacement, ReplacementData};

#[derive(Debug, Clone, Copy)]
pub struct ApkWriteOptions {
    pub strip_signatures: bool,
    /// Threads serializing and deflating dirty DEX files concurrently. Each
    /// holds one DEX's writer state, so this bounds the write-phase memory.
    pub dex_workers: NonZeroUsize,
    /// Deflate level for rewritten DEX entries. Level 3 compresses within two
    /// percent of level 6 in a quarter less time.
    pub dex_compression_level: i64,
}

impl Default for ApkWriteOptions {
    fn default() -> Self {
        Self {
            strip_signatures: true,
            dex_workers: std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN),
            dex_compression_level: 3,
        }
    }
}

pub(super) struct DexJob {
    pub dex_index: usize,
    pub component: usize,
    pub name: String,
}

struct WritePlan {
    jobs: Vec<DexJob>,
    /// After redistribution the new DEX set replaces every original entry.
    remove_originals: bool,
}

impl ApkFile {
    /// Writes every component into `output_dir` under its original file name
    /// and returns the paths written. The session stays usable: dirty state is
    /// kept, so a later write produces the same output again.
    #[instrument(level = "info", skip_all, fields(output_dir = %output_dir.as_ref().display()))]
    pub fn write_to(
        &mut self,
        output_dir: impl AsRef<Path>,
        options: ApkWriteOptions,
    ) -> Result<Vec<PathBuf>> {
        let output_dir = output_dir.as_ref();
        std::fs::create_dir_all(output_dir)?;
        let paths: Vec<PathBuf> = self
            .output_names()
            .iter()
            .map(|name| output_dir.join(name))
            .collect();
        self.write_components(options, |index| File::create(&paths[index]))?;
        Ok(paths)
    }

    /// Writes every component into an unlinked temp file under `dir` and
    /// returns them with their output file names. Nothing touches the file
    /// system by name, so an interrupted run leaves no partial output behind;
    /// `dir` lets the caller link a finished file into place later.
    #[instrument(level = "info", skip_all)]
    pub fn write_unsigned_files(
        &mut self,
        options: ApkWriteOptions,
        dir: &Path,
    ) -> Result<Vec<(String, File)>> {
        let names = self.output_names();
        let mut files: Vec<Option<File>> = (0..names.len()).map(|_| None).collect();
        self.write_components(options, |index| {
            let file = tempfile::tempfile_in(dir)?;
            files[index] = Some(file.try_clone()?);
            Ok(file)
        })?;
        Ok(names
            .into_iter()
            .zip(files)
            .map(|(name, file)| (name, file.expect("every component was written")))
            .collect())
    }

    fn output_names(&self) -> Vec<String> {
        self.components
            .iter()
            .map(|component| {
                component
                    .path()
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "output.apk".to_string())
            })
            .collect()
    }

    /// Serializes every component into the file `open` returns for it. Dirty
    /// DEX files are serialized and deflated in parallel workers and copied
    /// into the output as they complete, so no serialized DEX sits in memory.
    fn write_components(
        &mut self,
        options: ApkWriteOptions,
        mut open: impl FnMut(usize) -> io::Result<File>,
    ) -> Result<()> {
        let plan = self.write_plan()?;
        info!(
            dex_entry_count = self.dex.len(),
            dex_rewrite_count = plan.jobs.len(),
            strip_signatures = options.strip_signatures,
            "serializing APK output"
        );
        let pool = DexWorkPool::new(
            &self.dex.dex_files,
            &plan.jobs,
            options.dex_compression_level,
        );
        std::thread::scope(|scope| {
            let mut entries = DexEntryStream::start(scope, &pool, options.dex_workers.get());
            for (index, component) in self.components.iter().enumerate() {
                write_component(
                    component,
                    index,
                    &plan,
                    &mut entries,
                    open(index)?,
                    options.strip_signatures,
                )?;
            }
            Ok(())
        })
    }

    fn write_plan(&mut self) -> Result<WritePlan> {
        let any_dirty = self
            .dex_origins
            .iter()
            .any(|origin| matches!(origin, DexOrigin::Added))
            || self.dex.iter().any(DexFile::is_dirty);
        if !any_dirty {
            return Ok(WritePlan {
                jobs: Vec::new(),
                remove_originals: false,
            });
        }
        if self.dex.redistribute_if_needed()? {
            let jobs = (0..self.dex.len())
                .map(|dex_index| DexJob {
                    dex_index,
                    component: 0,
                    name: dex_entry_name(dex_index as u32 + 1),
                })
                .collect();
            return Ok(WritePlan {
                jobs,
                remove_originals: true,
            });
        }
        let mut used: HashSet<String> = self.base().original_dex_names().iter().cloned().collect();
        let dex_files = &self.dex.dex_files;
        let jobs = self
            .dex_origins
            .iter()
            .enumerate()
            .filter_map(|(dex_index, origin)| match origin {
                DexOrigin::Existing { component, name } => {
                    dex_files[dex_index].is_dirty().then(|| DexJob {
                        dex_index,
                        component: *component,
                        name: name.clone(),
                    })
                }
                DexOrigin::Added => Some(DexJob {
                    dex_index,
                    component: 0,
                    name: next_free_dex_name(&mut used),
                }),
            })
            .collect();
        Ok(WritePlan {
            jobs,
            remove_originals: false,
        })
    }
}

fn write_component(
    component: &ApkComponent,
    index: usize,
    plan: &WritePlan,
    entries: &mut DexEntryStream,
    output: File,
    strip_signatures: bool,
) -> Result<()> {
    debug!(component = component.name(), "writing APK component");
    let mut source = component.archive().clone();
    let manifest = component.manifest_bytes()?;
    let resources = component.resources_file()?;

    let mut removals = component.deleted().clone();
    if strip_signatures {
        removals.extend(
            source
                .file_names()
                .filter(|name| is_signature_entry(name))
                .map(String::from),
        );
    }
    if plan.remove_originals {
        removals.extend(component.original_dex_names().iter().cloned());
    }

    let mut replacements = BTreeMap::new();
    if let Some(bytes) = &manifest {
        replacements.insert(
            MANIFEST_ENTRY,
            Replacement {
                data: ReplacementData::Bytes(bytes),
                compression: zip::CompressionMethod::Deflated,
            },
        );
    }
    if let Some(file) = &resources {
        replacements.insert(
            RESOURCES_ENTRY,
            Replacement {
                data: ReplacementData::File(file),
                compression: zip::CompressionMethod::Stored,
            },
        );
    }
    for (name, (data, compression)) in component.injected() {
        replacements.insert(
            name.as_str(),
            Replacement {
                data: ReplacementData::Bytes(data),
                compression: compression.method(),
            },
        );
    }

    let dex_names: Vec<String> = plan
        .jobs
        .iter()
        .filter(|job| job.component == index)
        .map(|job| job.name.clone())
        .collect();
    let mut writer = ApkWriter::new(output);
    writer.rewrite(&mut source, &replacements, &removals, &dex_names, |name| {
        entries.take(index, name)
    })?;
    writer.finish()?;
    Ok(())
}
