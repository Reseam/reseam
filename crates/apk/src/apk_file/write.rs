// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use reseam_dex::DexPart;
use tracing::{debug, info, instrument};

use super::dex_workers::{DexEntryStream, DexWorkPool};
use super::{ApkComponent, ApkFile, DexOrigin};
use crate::entry::{is_signature_entry, next_free_dex_name, MANIFEST_ENTRY, RESOURCES_ENTRY};
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

/// One DEX entry to serialize: a whole DEX file, or one part of a DEX that
/// outgrew the id limits.
pub(super) struct DexJob {
    pub dex_index: usize,
    pub part: Option<DexPart>,
    pub component: usize,
    pub name: String,
}

impl ApkFile {
    /// Writes every component into `output_dir` under its original file name
    /// and returns the paths written. The session stays usable: dirty state is
    /// kept, so a later write produces the same output again.
    #[instrument(level = "info", skip_all, fields(output_dir = %output_dir.as_ref().display()))]
    pub fn write_to(
        &self,
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
        &self,
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
        &self,
        options: ApkWriteOptions,
        mut open: impl FnMut(usize) -> io::Result<File>,
    ) -> Result<()> {
        let jobs = self.dex_jobs()?;
        info!(
            dex_entry_count = self.dex.len(),
            dex_rewrite_count = jobs.len(),
            strip_signatures = options.strip_signatures,
            "serializing APK output"
        );
        let pool = DexWorkPool::new(&self.dex.dex_files, &jobs, options.dex_compression_level);
        std::thread::scope(|scope| {
            let mut entries = DexEntryStream::start(scope, &pool, options.dex_workers.get());
            for (index, component) in self.components.iter().enumerate() {
                write_component(
                    component,
                    index,
                    &jobs,
                    &mut entries,
                    open(index)?,
                    options.strip_signatures,
                )?;
            }
            Ok(())
        })
    }

    /// The DEX entries to serialize: every dirty or added DEX, a DEX that
    /// outgrew the id limits as several parts. The first part keeps the
    /// entry's name; later parts take the next free names in its component.
    fn dex_jobs(&self) -> Result<Vec<DexJob>> {
        let mut used: Vec<HashSet<String>> = self
            .components
            .iter()
            .map(|component| component.original_dex_names().iter().cloned().collect())
            .collect();
        let mut jobs = Vec::new();
        for (dex_index, (dex, origin)) in self.dex.iter().zip(&self.dex_origins).enumerate() {
            let (component, name) = match origin {
                DexOrigin::Existing { component, name } if dex.is_dirty() => {
                    (*component, name.clone())
                }
                DexOrigin::Existing { .. } => continue,
                DexOrigin::Added => (0, next_free_dex_name(&mut used[0])),
            };
            let Some(parts) = reseam_dex::split_to_fit(dex)? else {
                jobs.push(DexJob {
                    dex_index,
                    part: None,
                    component,
                    name,
                });
                continue;
            };
            info!(
                entry = name,
                parts = parts.len(),
                "splitting overflowed DEX"
            );
            for (ordinal, part) in parts.into_iter().enumerate() {
                let name = match ordinal {
                    0 => name.clone(),
                    _ => next_free_dex_name(&mut used[component]),
                };
                jobs.push(DexJob {
                    dex_index,
                    part: Some(part),
                    component,
                    name,
                });
            }
        }
        Ok(jobs)
    }
}

fn write_component(
    component: &ApkComponent,
    index: usize,
    jobs: &[DexJob],
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

    let dex_names: Vec<String> = jobs
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
