// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};

use reseam_dex::DexFile;
use tracing::{debug, info, instrument};

use super::{
    ApkComponentSession, ApkEntryPath, ApkFile, ApkWriteOptions, DexEntryOrigin, DexSessionEntry,
};
use crate::error::{invalid, Result};
use crate::zip::writer::{ApkReplacement, ApkWriter, ReplacementData};

/// One DEX to serialize into `component` under `name`.
struct DexJob {
    dex_index: usize,
    component_index: usize,
    name: ApkEntryPath,
}

struct DexWritePlan {
    jobs: Vec<DexJob>,
    /// Original DEX entries to drop from every component (after redistribution
    /// the new set replaces them all).
    remove_originals: bool,
    origins_after_write: Vec<DexEntryOrigin>,
}

impl ApkFile {
    /// Write the (possibly modified) APK to an output directory.
    ///
    /// For single APKs: writes one file at `output_dir/<original_name>`.
    /// For split bundles: writes base + all splits into `output_dir/`.
    ///
    /// Unchanged entries are copied through from the source APKs. Dirty DEX
    /// files are serialized and deflated in parallel workers and copied into
    /// the output as they complete, so no serialized DEX is held in memory.
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
        let output_paths: Vec<PathBuf> = self
            .output_names()
            .into_iter()
            .map(|name| output_dir.join(name))
            .collect();
        let plan = self.write_components(options, |index| File::create(&output_paths[index]))?;
        self.commit_after_write(output_paths, plan)?;
        info!("APK write completed");
        Ok(())
    }

    /// Writes every component into an unlinked temp file under `dir` and
    /// returns them with their output file names. Nothing touches the file
    /// system by name, so an interrupted run leaves no partial output behind;
    /// `dir` lets the caller link a finished file into place later.
    #[instrument(level = "info", skip_all, fields(component_count = self.components.len(), strip_signatures = options.strip_signatures))]
    pub fn write_unsigned_files(mut self, options: ApkWriteOptions, dir: &Path) -> Result<Vec<(String, File)>> {
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
                    .meta
                    .path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "output.apk".to_string())
            })
            .collect()
    }

    /// Serializes every component into the file `open` returns for it.
    fn write_components(
        &mut self,
        options: ApkWriteOptions,
        mut open: impl FnMut(usize) -> std::io::Result<File>,
    ) -> Result<DexWritePlan> {
        let plan = self.build_dex_write_plan()?;
        info!(
            dex_entry_count = self.dex.len(),
            dex_rewrite_count = plan.jobs.len(),
            strip_signatures = options.strip_signatures,
            "serializing APK output"
        );
        let components = &self.components;
        let pool = DexWorkPool {
            dex_files: &self.dex.dex_files,
            jobs: &plan.jobs,
            next: AtomicUsize::new(0),
            compression_level: options.dex_compression_level,
        };
        std::thread::scope(|scope| -> Result<()> {
            let mut entries = DexEntryStream::start(scope, &pool, options.dex_workers.get());
            for (idx, component) in components.iter().enumerate() {
                let output = open(idx)?;
                Self::write_component(component, idx, &plan, &mut entries, output, options.strip_signatures)?;
            }
            Ok(())
        })?;
        Ok(plan)
    }

    fn build_dex_write_plan(&mut self) -> Result<DexWritePlan> {
        if !self.any_dex_dirty() {
            return Ok(DexWritePlan {
                jobs: Vec::new(),
                remove_originals: false,
                origins_after_write: Vec::new(),
            });
        }

        if self.dex.redistribute_if_needed()? {
            let jobs: Vec<DexJob> = (0..self.dex.len())
                .map(|dex_index| DexJob {
                    dex_index,
                    component_index: 0,
                    name: base_dex_name(dex_index).into(),
                })
                .collect();
            let origins_after_write = jobs.iter().map(DexJob::origin).collect();
            return Ok(DexWritePlan {
                jobs,
                remove_originals: true,
                origins_after_write,
            });
        }

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

        let mut jobs = Vec::new();
        let mut origins_after_write = Vec::with_capacity(self.dex_sessions.len());
        for (dex_index, session) in self.dex_sessions.iter().enumerate() {
            let dirty = self.dex.dex_files[dex_index].is_dirty();
            let origin = match (session.state, session.origin.clone()) {
                (super::DexEntryState::Existing, Some(origin)) => {
                    if dirty {
                        jobs.push(DexJob {
                            dex_index,
                            component_index: origin.component_index,
                            name: origin.entry_name.clone(),
                        });
                    }
                    origin
                }
                (super::DexEntryState::Added, _) => {
                    let name: ApkEntryPath = next_free_base_dex_name(&mut used_base_names).into();
                    jobs.push(DexJob {
                        dex_index,
                        component_index: 0,
                        name: name.clone(),
                    });
                    DexEntryOrigin {
                        component_index: 0,
                        entry_name: name,
                    }
                }
                (state, None) => {
                    return Err(invalid(
                        "dex session",
                        format!("{state:?} DEX entry {dex_index} is missing an origin"),
                    ))
                }
            };
            origins_after_write.push(origin);
        }

        Ok(DexWritePlan {
            jobs,
            remove_originals: false,
            origins_after_write,
        })
    }

    fn commit_after_write(&mut self, output_paths: Vec<PathBuf>, plan: DexWritePlan) -> Result<()> {
        for (component, output_path) in self.components.iter_mut().zip(output_paths) {
            component.finalize_write(output_path)?;
        }

        if !plan.jobs.is_empty() {
            self.dex_sessions = plan
                .origins_after_write
                .into_iter()
                .map(|origin| DexSessionEntry {
                    origin: Some(origin),
                    state: super::DexEntryState::Existing,
                })
                .collect();
        }
        for dex in &mut self.dex.dex_files {
            dex.mark_clean();
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
        plan: &DexWritePlan,
        entries: &mut DexEntryStream,
        output: File,
        strip_signatures: bool,
    ) -> Result<()> {
        debug!(component = %component.meta.name, "writing APK component");
        let source_file = File::open(&component.meta.path)?;
        let mut source = zip::ZipArchive::new(BufReader::new(source_file))?;

        let mut writer = ApkWriter::new(output);

        let mut replacements = BTreeMap::new();
        let mut removals = HashSet::new();
        let manifest_bytes = if component.manifest_dirty {
            Some(component.manifest.serialize()?)
        } else {
            None
        };
        let resource_file = if component.resources_dirty {
            component
                .resources_loaded()
                .map(|resources| resources.serialize_spooled())
                .transpose()?
        } else {
            None
        };

        if strip_signatures {
            for index in 0..source.len() {
                let name = source.by_index_raw(index)?.name().to_string();
                if is_signature_entry(&name) {
                    removals.insert(name);
                }
            }
        }
        if plan.remove_originals {
            for name in &component.meta.original_dex_names {
                removals.insert(name.to_string());
            }
        }

        if let Some(manifest_bytes) = manifest_bytes.as_deref() {
            replacements.insert(
                "AndroidManifest.xml",
                ApkReplacement {
                    data: ReplacementData::Bytes(manifest_bytes),
                    compression: zip::CompressionMethod::Deflated,
                },
            );
        }

        if let Some(resource_file) = resource_file.as_ref() {
            replacements.insert(
                "resources.arsc",
                ApkReplacement {
                    data: ReplacementData::File(resource_file),
                    compression: zip::CompressionMethod::Stored,
                },
            );
        }

        for (name, (data, method)) in &component.injected_files {
            replacements.insert(
                name.as_str(),
                ApkReplacement {
                    data: ReplacementData::Bytes(data.as_slice()),
                    compression: *method,
                },
            );
        }
        for name in &component.deleted_files {
            removals.insert(name.to_string());
        }

        let dex_names: Vec<String> = plan
            .jobs
            .iter()
            .filter(|job| job.component_index == component_index)
            .map(|job| job.name.to_string())
            .collect();

        writer.rewrite_apk(&mut source, &replacements, &removals, &dex_names, |name| {
            entries.take(component_index, name)
        })?;
        writer.finish()?;
        Ok(())
    }
}

impl DexJob {
    fn origin(&self) -> DexEntryOrigin {
        DexEntryOrigin {
            component_index: self.component_index,
            entry_name: self.name.clone(),
        }
    }
}

/// The DEX files to serialize, handed out to workers one job at a time in
/// job order.
struct DexWorkPool<'a> {
    dex_files: &'a [DexFile],
    jobs: &'a [DexJob],
    next: AtomicUsize,
    compression_level: i64,
}

impl DexWorkPool<'_> {
    fn run(&self, sender: SyncSender<(usize, Result<File>)>) {
        loop {
            let job = self.next.fetch_add(1, Ordering::Relaxed);
            let Some(job_spec) = self.jobs.get(job) else {
                return;
            };
            let dex = &self.dex_files[job_spec.dex_index];
            let result = compress_dex(dex, &job_spec.name, self.compression_level);
            let failed = result.is_err();
            if sender.send((job, result)).is_err() || failed {
                return;
            }
        }
    }
}

/// Serialized, deflated DEX entries arriving from the worker pool.
///
/// Workers take jobs in order and each holds at most one finished entry, so
/// the writer waits on the entry it needs next while a bounded number of later
/// ones queue up.
struct DexEntryStream {
    receiver: Option<Receiver<(usize, Result<File>)>>,
    ready: HashMap<usize, File>,
    by_name: HashMap<(usize, String), usize>,
}

impl DexEntryStream {
    fn start<'scope, 'env: 'scope>(
        scope: &'scope std::thread::Scope<'scope, 'env>,
        pool: &'env DexWorkPool<'env>,
        workers: usize,
    ) -> Self {
        let by_name = pool
            .jobs
            .iter()
            .enumerate()
            .map(|(i, job)| ((job.component_index, job.name.to_string()), i))
            .collect();
        if pool.jobs.is_empty() {
            return Self {
                receiver: None,
                ready: HashMap::new(),
                by_name,
            };
        }

        let workers = workers.min(pool.jobs.len());
        let (sender, receiver) = sync_channel(workers);
        for _ in 0..workers {
            let sender = sender.clone();
            scope.spawn(move || pool.run(sender));
        }
        Self {
            receiver: Some(receiver),
            ready: HashMap::new(),
            by_name,
        }
    }

    fn take(&mut self, component_index: usize, name: &str) -> Result<Option<File>> {
        let Some(&job) = self.by_name.get(&(component_index, name.to_string())) else {
            return Ok(None);
        };
        loop {
            if let Some(bytes) = self.ready.remove(&job) {
                return Ok(Some(bytes));
            }
            let receiver = self
                .receiver
                .as_ref()
                .ok_or_else(|| invalid("dex write", "no DEX workers are running"))?;
            let (finished, result) = receiver
                .recv()
                .map_err(|_| invalid("dex write", "DEX worker stopped early"))?;
            self.ready.insert(finished, result?);
        }
    }
}

/// Serializes a DEX to a spooled file and deflates it into a single-entry
/// archive in another spooled file, whose compressed bytes the APK writer
/// copies verbatim. Neither the DEX nor its deflated form touches the heap.
fn compress_dex(dex: &DexFile, name: &ApkEntryPath, level: i64) -> Result<File> {
    let started = std::time::Instant::now();
    let spooled = reseam_dex::write_spooled(dex)?;
    let serialized = started.elapsed();
    let mapped = spooled.map()?;
    let mut archive = zip::ZipWriter::new(BufWriter::new(tempfile::tempfile()?));
    archive.start_file(
        name.as_str(),
        zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(level)),
    )?;
    archive.write_all(&mapped)?;
    let file = archive.finish()?.into_inner().map_err(|e| e.into_error())?;
    debug!(
        entry = name.as_str(),
        bytes = spooled.len(),
        serialize_ms = serialized.as_millis() as u64,
        deflate_ms = (started.elapsed() - serialized).as_millis() as u64,
        "dex entry written"
    );
    Ok(file)
}

fn base_dex_name(index: usize) -> String {
    if index == 0 {
        "classes.dex".to_string()
    } else {
        format!("classes{}.dex", index + 1)
    }
}

fn next_free_base_dex_name(used_names: &mut HashSet<String>) -> String {
    let mut index = 0;
    loop {
        let candidate = base_dex_name(index);
        if used_names.insert(candidate.clone()) {
            return candidate;
        }
        index += 1;
    }
}

fn dedupe_entry_names(entries: Vec<ApkEntryPath>) -> Vec<ApkEntryPath> {
    let mut seen = HashSet::new();
    entries
        .into_iter()
        .filter(|entry| seen.insert(entry.clone()))
        .collect()
}

fn is_signature_entry(name: &str) -> bool {
    let uppercase = name.to_ascii_uppercase();
    uppercase == "META-INF/MANIFEST.MF"
        || uppercase.ends_with(".SF")
        || uppercase.ends_with(".RSA")
        || uppercase.ends_with(".DSA")
        || uppercase.ends_with(".EC")
}
