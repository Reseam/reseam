// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod jobs;
mod layout;

use std::collections::HashSet;
use std::fs::File;
use std::io;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use tracing::{debug, info, instrument};

use super::baseline_profile::ProfileRewrite;
use super::component::EntryEdit;
use super::dex_workers::{DexEntry, DexEntryStream};
use super::{ApkFile, ComponentIndex};
use crate::error::{Result, invalid};
use crate::zip::writer::{Encoding, Writer};
pub(super) use jobs::{DexJob, DexMember};
use layout::{EntrySource, PlannedEntry, plan};

#[derive(Debug, Clone, Copy)]
pub struct ApkWriteOptions {
    /// Treatment of v1 signature entries. Original v2/v3 signing blocks are
    /// never copied by the ZIP writer.
    pub signatures: SignaturePolicy,
    /// Threads serializing and deflating dirty DEX files concurrently. Each
    /// holds one DEX's writer state and pages through its source, so this
    /// bounds the write-phase memory. Past four, the write phase got no
    /// faster on large multi-DEX apps while its peak kept growing.
    pub dex_workers: NonZeroUsize,
    /// Deflate level for rewritten DEX entries. Level 3 compresses within two
    /// percent of level 6 in a quarter less time.
    pub dex_compression_level: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SignaturePolicy {
    #[default]
    Strip,
    /// Retains v1 entries verbatim, including signatures invalidated by edits.
    Preserve,
}

impl Default for ApkWriteOptions {
    fn default() -> Self {
        Self {
            signatures: SignaturePolicy::Strip,
            dex_workers: std::thread::available_parallelism()
                .unwrap_or(NonZeroUsize::MIN)
                .min(NonZeroUsize::MIN.saturating_add(3)),
            dex_compression_level: 3,
        }
    }
}

impl ApkFile {
    /// Writes complete component APKs under their input file names. Output is
    /// staged in temporary files before publication. Duplicate output names and
    /// paths pointing to the input are errors. Dirty state is retained, so the
    /// session can be written repeatedly.
    /// Edited v041 containers retain their physical entry and logical member
    /// order, including any pool-sized parts required by overflow.
    #[instrument(level = "info", skip_all)]
    pub fn write_to(
        &self,
        output_dir: impl AsRef<Path>,
        options: ApkWriteOptions,
    ) -> Result<Vec<PathBuf>> {
        let output_dir = output_dir.as_ref();
        std::fs::create_dir_all(output_dir)?;
        let names = self.output_names()?;
        let paths: Vec<_> = names.iter().map(|name| output_dir.join(name)).collect();
        for path in &paths {
            if path.exists() {
                let destination = reseam_storage::canonicalize(path)?;
                for component in &self.components {
                    if destination == reseam_storage::canonicalize(component.path())? {
                        return Err(invalid(
                            "apk output",
                            format!("{} is an input APK", path.display()),
                        ));
                    }
                }
            }
        }
        let staging = paths
            .iter()
            .map(|_| tempfile::NamedTempFile::new_in(output_dir))
            .collect::<io::Result<Vec<_>>>()?;
        self.write_components(
            options,
            staging
                .iter()
                .map(tempfile::NamedTempFile::reopen)
                .collect::<io::Result<_>>()?,
        )?;
        for (file, path) in staging.into_iter().zip(&paths) {
            file.persist(path).map_err(|error| error.error)?;
        }
        Ok(paths)
    }

    /// Writes each component into an unlinked file in `dir`. The caller owns
    /// publication; the returned files are positioned at the end of the ZIP.
    #[instrument(level = "info", skip_all)]
    pub fn write_unsigned_files(
        &self,
        options: ApkWriteOptions,
        dir: &Path,
    ) -> Result<Vec<(String, File)>> {
        let names = self.output_names()?;
        let files = self.write_components(
            options,
            names
                .iter()
                .map(|_| tempfile::tempfile_in(dir))
                .collect::<io::Result<_>>()?,
        )?;
        Ok(names.into_iter().zip(files).collect())
    }

    fn output_names(&self) -> Result<Vec<String>> {
        let mut used = HashSet::new();
        self.components
            .iter()
            .map(|component| {
                let name = component
                    .path()
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| {
                        invalid(
                            "apk output",
                            format!("{} has no UTF-8 file name", component.path().display()),
                        )
                    })?;
                if !used.insert(name) {
                    return Err(invalid(
                        "apk output",
                        format!("duplicate output file name {name}"),
                    ));
                }
                Ok(name.to_string())
            })
            .collect()
    }

    /// Writes each component into its output, in component order. Components
    /// without rewritten DEX go first, so their copying overlaps serialization.
    fn write_components(&self, options: ApkWriteOptions, outputs: Vec<File>) -> Result<Vec<File>> {
        let mut jobs = self.dex_jobs()?;
        jobs.sort_by_key(|job| {
            (
                job.component,
                self.components[job.component.0]
                    .archive()
                    .index_for_name(job.name.as_str())
                    .unwrap_or(usize::MAX),
            )
        });
        let profiles = (0..self.components.len())
            .map(|index| ProfileRewrite::new(self, ComponentIndex(index), &jobs))
            .collect::<Result<Vec<_>>>()?;
        let plans: Vec<_> = self
            .components
            .iter()
            .zip(&profiles)
            .enumerate()
            .map(|(index, (component, profile))| {
                plan(
                    component,
                    ComponentIndex(index),
                    &jobs,
                    options.signatures,
                    profile.is_some(),
                )
            })
            .collect();
        let mut order: Vec<_> = (0..self.components.len()).map(ComponentIndex).collect();
        order.sort_by_key(|&index| jobs.iter().any(|job| job.component == index));
        info!(
            dex_entry_count = self.dex.len(),
            dex_rewrite_count = jobs.len(),
            strip_signatures = options.signatures == SignaturePolicy::Strip,
            "serializing APK output"
        );
        std::thread::scope(|scope| {
            let mut entries = DexEntryStream::start(
                scope,
                &self.dex,
                &jobs,
                options.dex_workers.get(),
                options.dex_compression_level,
            )?;
            let mut profiles = profiles;
            let mut files: Vec<_> = outputs.into_iter().map(Some).collect();
            for index in order {
                let output = files[index.0]
                    .take()
                    .expect("each component is written once");
                files[index.0] = Some(self.write_component(
                    index,
                    &plans[index.0],
                    &jobs,
                    profiles[index.0].take(),
                    &mut entries,
                    output,
                )?);
            }
            Ok(files.into_iter().flatten().collect())
        })
    }

    fn write_component(
        &self,
        index: ComponentIndex,
        plan: &[PlannedEntry<'_>],
        jobs: &[DexJob],
        mut profile: Option<ProfileRewrite>,
        entries: &mut DexEntryStream<'_>,
        output: File,
    ) -> Result<File> {
        let component = &self.components[index.0];
        debug!(component = component.name(), "writing APK component");
        let mut source = component.archive().clone();
        let source_file = File::open(component.path())?;
        let mut writer = Writer::new(output);
        for entry in plan {
            let result = match entry.source {
                EntrySource::Archive(index) => {
                    writer.copy_original(&mut source, &source_file, index)
                }
                EntrySource::Dex(job) => entries.next().and_then(|written| {
                    let checksum = match written.entry {
                        DexEntry::Deflated(file) => writer.copy_compressed(&file)?,
                        DexEntry::Stored { file, crc } => {
                            writer.write_stored(entry.name.as_str(), &file, crc)?;
                            crc
                        }
                    };
                    if let Some(profile) = &mut profile {
                        profile.record(self, index, &jobs[job], &written.layouts, checksum);
                    }
                    Ok(())
                }),
                EntrySource::Edit(EntryEdit::Staged { file, compression }) => {
                    writer.write_file(entry.name.as_str(), file, compression.encoding())
                }
                EntrySource::Edit(EntryEdit::Manifest) => writer.write_bytes(
                    entry.name.as_str(),
                    &component.manifest().serialize()?,
                    Encoding::DEFLATED,
                ),
                EntrySource::Edit(EntryEdit::Resources) => writer.write_file(
                    entry.name.as_str(),
                    &component.resources_file()?,
                    Encoding::Stored,
                ),
                EntrySource::Edit(EntryEdit::Deleted | EntryEdit::Dex) => {
                    Err(invalid("apk write", "entry has no planned source"))
                }
            };
            result.map_err(|error| error.in_entry(entry.name.as_str()))?;
        }
        if let Some(profile) = profile {
            for (name, bytes) in profile.finish().to_entries()? {
                writer
                    .write_bytes(name, &bytes, Encoding::Stored)
                    .map_err(|error| error.in_entry(name))?;
            }
        }
        writer.finish()
    }
}
