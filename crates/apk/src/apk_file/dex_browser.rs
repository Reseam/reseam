// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::VecDeque;
use std::fs::File;
use std::os::wasi::io::AsRawFd;
use std::thread::Scope;

use reseam_dex::DexFile;

use super::{DexJob, serialize_dex};
use crate::error::{Result, invalid};

#[link(wasm_import_module = "reseam_compression")]
unsafe extern "C" {
    fn workers() -> u32;
    fn submit(input: i32, output: i32, name: *const u8, length: usize, level: i64) -> u32;
    fn finish(ticket: u32) -> u32;
}

struct Pending {
    ticket: Option<u32>,
    // Files outlive their worker job, including when a preceding job fails.
    _input: File,
    output: Option<File>,
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
    dex: &'a [DexFile],
    jobs: &'a [DexJob],
    pending: VecDeque<Pending>,
    scheduled: usize,
    next: usize,
    level: i64,
}

impl<'a> DexEntryStream<'a> {
    pub fn start<'scope, 'env>(
        _scope: &'scope Scope<'scope, 'env>,
        dex: &'a [DexFile],
        jobs: &'a [DexJob],
        _workers: usize,
        level: i64,
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
        let input = serialize_dex(self.dex, &job.members)
            .map_err(|error| error.in_entry(job.name.as_str()))?
            .into_file();
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
                self.level,
            )
        };
        self.pending.push_back(Pending {
            ticket: Some(ticket),
            _input: input,
            output: Some(output),
        });
        self.scheduled += 1;
        Ok(())
    }

    pub fn next(&mut self) -> Result<File> {
        let mut pending = self
            .pending
            .pop_front()
            .ok_or_else(|| invalid("dex write", "no pending DEX entry"))?;
        pending
            .finish()
            .map_err(|error| error.in_entry(self.jobs[self.next].name.as_str()))?;
        let file = pending
            .output
            .take()
            .expect("completed compression owns its output");
        self.next += 1;
        self.schedule()?;
        Ok(file)
    }
}
