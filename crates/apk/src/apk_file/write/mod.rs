// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod dex_stream;
mod jobs;
mod layout;
mod profile;
mod publish;

use std::fs::File;
use std::num::NonZeroUsize;

use tracing::{debug, info};

use super::component::EntryEdit;
use super::{ApkFile, ComponentIndex};
use crate::error::{Result, invalid};
use crate::zip::Compression;
use crate::zip::writer::{DEFAULT_LEVEL, Writer};
use dex_stream::{DexEntry, DexEntryStream};
pub(super) use jobs::DexJob;
use layout::{EntrySource, PlannedEntry, plan};
use profile::ProfileRewrite;

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
                    profile.as_ref(),
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
        let mut writer = Writer::new(output, DEFAULT_LEVEL);
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
                    writer.write(entry.name.as_str(), file, *compression)
                }
                EntrySource::Edit(EntryEdit::Manifest) => writer.write(
                    entry.name.as_str(),
                    component.manifest().serialize()?.as_slice(),
                    Compression::Deflated,
                ),
                EntrySource::Edit(EntryEdit::Resources) => writer.write(
                    entry.name.as_str(),
                    &component.resources_file()?,
                    Compression::Stored,
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
                    .write(name, bytes.as_slice(), Compression::Stored)
                    .map_err(|error| error.in_entry(name))?;
            }
        }
        writer.finish()
    }
}
