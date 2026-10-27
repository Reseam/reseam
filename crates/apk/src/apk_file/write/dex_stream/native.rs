// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread::Scope;

use tracing::debug;

use super::{DexEntry, WrittenDex, stored_entry};
use crate::apk_file::dex_set::DexSet;
use crate::apk_file::write::DexJob;
use crate::error::{Result, invalid};
use crate::zip::COPY_BUFFER;
use crate::zip::Compression;

pub(in crate::apk_file) struct DexEntryStream<'a> {
    _lifetime: std::marker::PhantomData<&'a ()>,
    pending: SyncSender<usize>,
    receivers: Vec<Receiver<Result<WrittenDex>>>,
    next: usize,
    scheduled: usize,
    job_count: usize,
}

impl DexEntryStream<'_> {
    pub fn start<'scope, 'env>(
        scope: &'scope Scope<'scope, 'env>,
        dex: &'env DexSet,
        jobs: &'env [DexJob],
        workers: usize,
        level: u32,
    ) -> Result<Self> {
        let workers = workers.min(jobs.len());
        let window = workers.saturating_mul(2).min(jobs.len());
        let (pending, queue) = sync_channel::<usize>(window);
        let queue = Arc::new(Mutex::new(queue));
        let (outputs, receivers): (Vec<_>, Vec<_>) = (0..window)
            .map(|_| sync_channel::<Result<WrittenDex>>(1))
            .unzip();
        let outputs = Arc::new(outputs);
        for _ in 0..workers {
            let queue = Arc::clone(&queue);
            let outputs = Arc::clone(&outputs);
            std::thread::Builder::new().spawn_scoped(scope, move || {
                let mut buffer = vec![0; COPY_BUFFER];
                loop {
                    let Ok(index) = queue
                        .lock()
                        .expect("queue lock is never held while executing a job")
                        .recv()
                    else {
                        break;
                    };
                    let job = &jobs[index];
                    let result = write_dex(dex, job, level, &mut buffer)
                        .map_err(|error| error.in_entry(job.name.as_str()));
                    let failed = result.is_err();
                    if outputs[index % outputs.len()].send(result).is_err() || failed {
                        break;
                    }
                }
            })?;
        }
        for index in 0..window {
            pending
                .send(index)
                .expect("initial jobs fit in the queue while its receiver is retained");
        }
        Ok(Self {
            _lifetime: std::marker::PhantomData,
            pending,
            receivers,
            next: 0,
            scheduled: window,
            job_count: jobs.len(),
        })
    }

    pub fn next(&mut self) -> Result<WrittenDex> {
        let receiver = self
            .receivers
            .get(self.next % self.receivers.len().max(1))
            .ok_or_else(|| invalid("dex write", "no DEX workers are running"))?;
        let written = receiver
            .recv()
            .map_err(|_| invalid("dex write", "DEX worker stopped early"))??;
        self.next += 1;
        if self.scheduled < self.job_count {
            self.pending
                .send(self.scheduled)
                .map_err(|_| invalid("dex write", "DEX workers stopped early"))?;
            self.scheduled += 1;
        }
        Ok(written)
    }
}

fn write_dex(dex: &DexSet, job: &DexJob, level: u32, buffer: &mut [u8]) -> Result<WrittenDex> {
    let started = std::time::Instant::now();
    let (spooled, layouts) = dex.serialize(&job.members)?;
    let serialized = started.elapsed();
    let compressing = std::time::Instant::now();
    let bytes = spooled.len();
    let entry = match job.compression {
        Compression::Stored => stored_entry(spooled, buffer)?,
        Compression::Deflated => DexEntry::Deflated(crate::compression::compress_dex_entry(
            &spooled.into_file(),
            reseam_storage::temporary_file()?,
            job.name.as_str(),
            level,
        )?),
    };
    debug!(
        entry = job.name.as_str(),
        bytes,
        serialize_ms = serialized.as_millis() as u64,
        compress_ms = compressing.elapsed().as_millis() as u64,
        "dex entry written"
    );
    Ok(WrittenDex { entry, layouts })
}
