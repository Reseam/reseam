// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs::File;
use std::io::Read;
#[cfg(not(target_os = "wasi"))]
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
#[cfg(not(target_os = "wasi"))]
use std::sync::{Arc, Mutex};
#[cfg(not(target_os = "wasi"))]
use std::thread::Scope;

use reseam_dex::{DexFile, WriteLayout};
#[cfg(not(target_os = "wasi"))]
use tracing::debug;

use super::component::Compression;
use super::write::{DexJob, DexMember};
use crate::error::Result;
#[cfg(not(target_os = "wasi"))]
use crate::error::invalid;
#[cfg(not(target_os = "wasi"))]
use crate::zip::COPY_BUFFER;

/// A rewritten DEX ready to be added to its APK, with where each of its
/// members' pools and classes landed.
pub(super) struct WrittenDex {
    pub entry: DexEntry,
    pub layouts: Vec<WriteLayout>,
}

pub(super) enum DexEntry {
    /// A single-entry ZIP holding the deflated DEX.
    Deflated(File),
    /// The serialized DEX and its CRC-32.
    Stored { file: File, crc: u32 },
}

#[cfg(not(target_os = "wasi"))]
pub(super) struct DexEntryStream<'a> {
    _lifetime: std::marker::PhantomData<&'a ()>,
    pending: SyncSender<usize>,
    receivers: Vec<Receiver<Result<WrittenDex>>>,
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
                    let result = write_dex(dex_files, job, level, &mut buffer)
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

fn serialize_dex(
    dex_files: &[DexFile],
    members: &[DexMember],
) -> Result<(reseam_dex::Spooled, Vec<WriteLayout>)> {
    let written = match members {
        [member] => {
            let (spooled, layout) =
                reseam_dex::write_spooled(&dex_files[member.dex_index.0], member.part.as_ref())?;
            (spooled, vec![layout])
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
    Ok(written)
}

/// The entry for a stored job, which needs no compression worker. `buffer`
/// carries the bytes being checksummed.
fn stored_entry(spooled: reseam_dex::Spooled, buffer: &mut [u8]) -> Result<DexEntry> {
    let mut crc = flate2::Crc::new();
    let mut reader = spooled.reader();
    loop {
        let read = reader.read(buffer)?;
        if read == 0 {
            break;
        }
        crc.update(&buffer[..read]);
    }
    drop(reader);
    Ok(DexEntry::Stored {
        file: spooled.into_file(),
        crc: crc.sum(),
    })
}

#[cfg(not(target_os = "wasi"))]
fn write_dex(
    dex_files: &[DexFile],
    job: &DexJob,
    level: u32,
    buffer: &mut [u8],
) -> Result<WrittenDex> {
    let started = std::time::Instant::now();
    let (spooled, layouts) = serialize_dex(dex_files, &job.members)?;
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

#[cfg(target_os = "wasi")]
#[path = "dex_browser.rs"]
mod browser;
#[cfg(target_os = "wasi")]
pub(super) use browser::DexEntryStream;
