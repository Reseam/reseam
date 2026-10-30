// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Files a patch reads or writes a piece at a time, held open between calls
//! until the patch closes them or its invocation ends.

use std::cell::RefCell;
use std::fs::File;
use std::io::{BufWriter, Read, Write};

use boltffi::export;
use reseam_apk::{Compression, EntryReader};

use super::components::with_component;
use super::files::{FileCompression, check_read_length};
use crate::bridge::handles::OwnedHandles;
use crate::bridge::phase::with_ctx_result;

/// An entry being written, until the patch commits or discards it.
struct PendingWrite {
    component: usize,
    path: String,
    compression: Compression,
    file: BufWriter<File>,
}

thread_local! {
    static READS: RefCell<OwnedHandles<EntryReader>> = RefCell::default();
    static WRITES: RefCell<OwnedHandles<PendingWrite>> = RefCell::default();
}

pub(in crate::bridge) fn reset() {
    READS.with(|reads| *reads.borrow_mut() = OwnedHandles::default());
    WRITES.with(|writes| *writes.borrow_mut() = OwnedHandles::default());
}

fn not_open(handle: u32) -> String {
    format!("file stream {handle} is not open")
}

#[export]
pub fn file_stream_open(
    component_name: Option<String>,
    apk_path: String,
) -> Result<Option<u32>, String> {
    with_component(component_name, |ctx, index| {
        let Some(entry) = ctx
            .apk_mut()
            .open_component_entry(index, &apk_path)
            .map_err(|error| format!("stream {apk_path}: {error}"))?
        else {
            return Ok(None);
        };
        READS
            .with(|reads| reads.borrow_mut().insert(entry))
            .map(Some)
    })
}

/// The next `length` bytes of stream `handle`, fewer only at its end.
#[export]
pub fn file_stream_read(handle: u32, length: u32) -> Result<Vec<u8>, String> {
    check_read_length(length)?;
    READS.with(|reads| {
        let mut reads = reads.borrow_mut();
        let entry = reads.get_mut(handle).ok_or_else(|| not_open(handle))?;
        let mut bytes = Vec::new();
        entry
            .by_ref()
            .take(u64::from(length))
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        Ok(bytes)
    })
}

#[export]
pub fn file_stream_close(handle: u32) -> Result<(), String> {
    READS
        .with(|reads| reads.borrow_mut().remove(handle))
        .map(drop)
        .ok_or_else(|| not_open(handle))
}

#[export]
pub fn file_write_open(
    component_name: Option<String>,
    apk_path: String,
    compression: FileCompression,
) -> Result<u32, String> {
    with_component(component_name, |_, component| {
        let file = reseam_storage::temporary_file().map_err(|error| error.to_string())?;
        WRITES.with(|writes| {
            writes.borrow_mut().insert(PendingWrite {
                component,
                path: apk_path,
                compression: compression.into(),
                file: BufWriter::new(file),
            })
        })
    })
}

#[export]
pub fn file_write_append(handle: u32, data: Vec<u8>) -> Result<(), String> {
    WRITES.with(|writes| {
        writes
            .borrow_mut()
            .get_mut(handle)
            .ok_or_else(|| not_open(handle))?
            .file
            .write_all(&data)
            .map_err(|error| error.to_string())
    })
}

/// Adds or replaces the entry with everything appended to `handle`.
#[export]
pub fn file_write_commit(handle: u32) -> Result<(), String> {
    let write = WRITES
        .with(|writes| writes.borrow_mut().remove(handle))
        .ok_or_else(|| not_open(handle))?;
    let path = write.path;
    let file = write
        .file
        .into_inner()
        .map_err(|error| format!("inject {path}: {}", error.into_error()))?;
    with_ctx_result(|ctx| {
        ctx.inject_spooled(write.component, &path, file, write.compression)
            .map_err(|error| format!("inject {path}: {error}"))
    })
}

#[export]
pub fn file_write_discard(handle: u32) -> Result<(), String> {
    WRITES
        .with(|writes| writes.borrow_mut().remove(handle))
        .map(drop)
        .ok_or_else(|| not_open(handle))
}
