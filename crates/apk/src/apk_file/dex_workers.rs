// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

#[cfg(not(target_os = "wasi"))]
use std::fs::File;
#[cfg(not(target_os = "wasi"))]
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
#[cfg(not(target_os = "wasi"))]
use std::sync::{Arc, Mutex};
#[cfg(not(target_os = "wasi"))]
use std::thread::Scope;

use reseam_dex::DexFile;
#[cfg(not(target_os = "wasi"))]
use tracing::debug;

use super::write::{DexJob, DexMember};
use crate::error::Result;
#[cfg(not(target_os = "wasi"))]
use crate::error::invalid;

#[cfg(not(target_os = "wasi"))]
pub(super) struct DexEntryStream<'a> {
    _lifetime: std::marker::PhantomData<&'a ()>,
    pending: SyncSender<usize>,
    receivers: Vec<Receiver<Result<File>>>,
    next: usize,
    scheduled: usize,
    job_count: usize,
}

#[cfg(not(target_os = "wasi"))]
impl DexEntryStream<'_> {
    pub fn start<'scope, 'env>(
        scope: &'scope Scope<'scope, 'env>,
        dex_files: &'env [DexFile],
        jobs: &'env [DexJob],
        workers: usize,
        level: i64,
    ) -> Result<Self> {
        let workers = workers.min(jobs.len());
        let window = workers.saturating_mul(2).min(jobs.len());
        let (pending, queue) = sync_channel::<usize>(window);
        let queue = Arc::new(Mutex::new(queue));
        let (outputs, receivers): (Vec<_>, Vec<_>) =
            (0..window).map(|_| sync_channel::<Result<File>>(1)).unzip();
        let outputs = Arc::new(outputs);
        for _ in 0..workers {
            let queue = Arc::clone(&queue);
            let outputs = Arc::clone(&outputs);
            std::thread::Builder::new().spawn_scoped(scope, move || {
                loop {
                    let Ok(index) = queue
                        .lock()
                        .expect("queue lock is never held while executing a job")
                        .recv()
                    else {
                        break;
                    };
                    let job = &jobs[index];
                    let result = compress_dex(dex_files, &job.members, job.name.as_str(), level)
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

    pub fn next(&mut self) -> Result<File> {
        let receiver = self
            .receivers
            .get(self.next % self.receivers.len().max(1))
            .ok_or_else(|| invalid("dex write", "no DEX workers are running"))?;
        let file = receiver
            .recv()
            .map_err(|_| invalid("dex write", "DEX worker stopped early"))??;
        self.next += 1;
        if self.scheduled < self.job_count {
            self.pending
                .send(self.scheduled)
                .map_err(|_| invalid("dex write", "DEX workers stopped early"))?;
            self.scheduled += 1;
        }
        Ok(file)
    }
}

fn serialize_dex(
    dex_files: &[DexFile],
    members: &[DexMember],
) -> Result<reseam_dex::write::Spooled> {
    let spooled = match members {
        [member] => {
            reseam_dex::write_spooled(&dex_files[member.dex_index.0], member.part.as_ref())?
        }
        _ => reseam_dex::write_container_spooled(
            members
                .iter()
                .map(|member| (&dex_files[member.dex_index.0], member.part.as_ref())),
        )?,
    };
    if let Some(member) = members.first() {
        dex_files[member.dex_index.0].release_pages();
    }
    Ok(spooled)
}

#[cfg(not(target_os = "wasi"))]
fn compress_dex(
    dex_files: &[DexFile],
    members: &[DexMember],
    name: &str,
    level: i64,
) -> Result<File> {
    let started = std::time::Instant::now();
    let spooled = serialize_dex(dex_files, members)?;
    let serialized = started.elapsed();
    let deflating = std::time::Instant::now();
    let file = crate::compression::compress_dex_entry(
        spooled.reader(),
        reseam_storage::temporary_file()?,
        name,
        level,
    )?;
    debug!(
        entry = name,
        bytes = spooled.len(),
        serialize_ms = serialized.as_millis() as u64,
        deflate_ms = deflating.elapsed().as_millis() as u64,
        "dex entry written"
    );
    Ok(file)
}

#[cfg(target_os = "wasi")]
#[path = "dex_browser.rs"]
mod browser;
#[cfg(target_os = "wasi")]
pub(super) use browser::DexEntryStream;
