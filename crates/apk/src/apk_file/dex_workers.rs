// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::thread::Scope;

use reseam_dex::DexFile;
use tracing::debug;

use super::write::DexJob;
use crate::error::{invalid, Result};

/// The DEX files to serialize, handed out to workers one job at a time in
/// job order.
pub(super) struct DexWorkPool<'a> {
    dex_files: &'a [DexFile],
    jobs: &'a [DexJob],
    next: AtomicUsize,
    compression_level: i64,
}

impl<'a> DexWorkPool<'a> {
    pub fn new(dex_files: &'a [DexFile], jobs: &'a [DexJob], compression_level: i64) -> Self {
        Self {
            dex_files,
            jobs,
            next: AtomicUsize::new(0),
            compression_level,
        }
    }

    fn run(&self, sender: SyncSender<(usize, Result<File>)>) {
        loop {
            let index = self.next.fetch_add(1, Ordering::Relaxed);
            let Some(job) = self.jobs.get(index) else {
                return;
            };
            let result = compress_dex(
                &self.dex_files[job.dex_index],
                &job.name,
                self.compression_level,
            );
            let failed = result.is_err();
            if sender.send((index, result)).is_err() || failed {
                return;
            }
        }
    }
}

/// Serialized, deflated DEX entries arriving from the worker pool. Workers
/// take jobs in order and each holds at most one finished entry, so the
/// writer waits on the entry it needs next while a bounded number of later
/// ones queue up.
pub(super) struct DexEntryStream<'a> {
    jobs: &'a [DexJob],
    receiver: Option<Receiver<(usize, Result<File>)>>,
    ready: HashMap<usize, File>,
}

impl<'a> DexEntryStream<'a> {
    pub fn start<'scope>(
        scope: &'scope Scope<'scope, 'a>,
        pool: &'a DexWorkPool<'a>,
        workers: usize,
    ) -> Self {
        let receiver = (!pool.jobs.is_empty()).then(|| {
            let workers = workers.min(pool.jobs.len());
            let (sender, receiver) = sync_channel(workers);
            for _ in 0..workers {
                let sender = sender.clone();
                scope.spawn(move || pool.run(sender));
            }
            receiver
        });
        Self {
            jobs: pool.jobs,
            receiver,
            ready: HashMap::new(),
        }
    }

    /// The finished entry for `name` in `component`, waiting for workers as
    /// needed; `None` when no job produces it.
    pub fn take(&mut self, component: usize, name: &str) -> Result<Option<File>> {
        let Some(job) = self
            .jobs
            .iter()
            .position(|job| job.component == component && job.name.as_str() == name)
        else {
            return Ok(None);
        };
        loop {
            if let Some(file) = self.ready.remove(&job) {
                return Ok(Some(file));
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
fn compress_dex(dex: &DexFile, name: &str, level: i64) -> Result<File> {
    let started = std::time::Instant::now();
    let spooled = reseam_dex::write_spooled(dex)?;
    let serialized = started.elapsed();
    let mapped = spooled.map()?;
    let mut archive = zip::ZipWriter::new(BufWriter::new(tempfile::tempfile()?));
    archive.start_file(
        name,
        zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(level)),
    )?;
    archive.write_all(&mapped)?;
    let file = archive.finish()?.into_inner().map_err(|e| e.into_error())?;
    debug!(
        entry = name,
        bytes = spooled.len(),
        serialize_ms = serialized.as_millis() as u64,
        deflate_ms = (started.elapsed() - serialized).as_millis() as u64,
        "dex entry written"
    );
    Ok(file)
}
