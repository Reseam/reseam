// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::VecDeque;
use std::fs::File;
use std::os::wasi::io::AsRawFd;
use std::thread::Scope;

use reseam_dex::WriteLayout;

use super::{DexEntry, WrittenDex, stored_entry};
use crate::apk_file::dex_set::DexSet;
use crate::apk_file::write::DexJob;
use crate::error::{Result, invalid};
use crate::zip::COPY_BUFFER;
use crate::zip::Compression;

#[link(wasm_import_module = "reseam_compression")]
unsafe extern "C" {
    fn workers() -> u32;
    fn submit(input: i32, output: i32, name: *const u8, length: usize, level: i64) -> u32;
    fn finish(ticket: u32) -> u32;
}

struct Pending {
    ticket: Option<u32>,
    // Files outlive their worker job, including when a preceding job fails.
    _input: Option<File>,
    entry: Option<DexEntry>,
    layouts: Vec<WriteLayout>,
}

impl Pending {
    fn finish(&mut self) -> Result<()> {
        if let Some(ticket) = self.ticket.take() {
            // SAFETY: the ticket belongs to this job and the files remain open.
            let errno = unsafe { finish(ticket) };
            if errno != 0 {
                return Err(std::io::Error::from_raw_os_error(errno as i32).into());
            }
        }
        Ok(())
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        if let Err(error) = self.finish() {
            tracing::warn!(%error, "failed to drain DEX compression job");
        }
    }
}

pub(in crate::apk_file) struct DexEntryStream<'a> {
    dex: &'a DexSet,
    jobs: &'a [DexJob],
    pending: VecDeque<Pending>,
    scheduled: usize,
    next: usize,
    level: u32,
    buffer: Vec<u8>,
}

impl<'a> DexEntryStream<'a> {
    pub fn start<'scope, 'env>(
        _scope: &'scope Scope<'scope, 'env>,
        dex: &'a DexSet,
        jobs: &'a [DexJob],
        _workers: usize,
        level: u32,
    ) -> Result<Self> {
        // SAFETY: the browser reports the bounded pool it initialized for this run.
        let count = unsafe { workers() } as usize;
        if count == 0 && !jobs.is_empty() {
            return Err(invalid("dex write", "no compression workers available"));
        }
        let mut stream = Self {
            dex,
            jobs,
            pending: VecDeque::new(),
            scheduled: 0,
            next: 0,
            level,
            buffer: vec![0; COPY_BUFFER],
        };
        for _ in 0..count.min(jobs.len()) {
            stream.schedule()?;
        }
        Ok(stream)
    }

    fn schedule(&mut self) -> Result<()> {
        let Some(job) = self.jobs.get(self.scheduled) else {
            return Ok(());
        };
        let (spooled, layouts) = self
            .dex
            .serialize(&job.members)
            .map_err(|error| error.in_entry(job.name.as_str()))?;
        let pending = match job.compression {
            Compression::Stored => Pending {
                ticket: None,
                _input: None,
                entry: Some(stored_entry(spooled, &mut self.buffer)?),
                layouts,
            },
            Compression::Deflated => {
                let input = spooled.into_file();
                let output = reseam_storage::temporary_file()?;
                let name = job.name.as_str();
                // SAFETY: the host copies name synchronously. The pending job owns both
                // descriptors until finish acknowledges that the worker stopped using them.
                let ticket = unsafe {
                    submit(
                        input.as_raw_fd(),
                        output.as_raw_fd(),
                        name.as_ptr(),
                        name.len(),
                        i64::from(self.level),
                    )
                };
                Pending {
                    ticket: Some(ticket),
                    _input: Some(input),
                    entry: Some(DexEntry::Deflated(output)),
                    layouts,
                }
            }
        };
        self.pending.push_back(pending);
        self.scheduled += 1;
        Ok(())
    }

    pub fn next(&mut self) -> Result<WrittenDex> {
        let mut pending = self
            .pending
            .pop_front()
            .ok_or_else(|| invalid("dex write", "no pending DEX entry"))?;
        pending
            .finish()
            .map_err(|error| error.in_entry(self.jobs[self.next].name.as_str()))?;
        let entry = pending
            .entry
            .take()
            .expect("a completed job owns its entry");
        let layouts = std::mem::take(&mut pending.layouts);
        self.next += 1;
        self.schedule()?;
        Ok(WrittenDex { entry, layouts })
    }
}
