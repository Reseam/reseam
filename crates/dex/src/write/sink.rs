// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::File;
use std::io;

use crate::error::{DexError, Result};
use reseam_storage::file::FileExt;

/// Destination of a DEX serialization. The writer appends sequentially and
/// backpatches tables it emitted earlier; how those land is up to the sink.
pub trait DexSink {
    fn pos(&self) -> u32;
    fn write(&mut self, bytes: &[u8]);
    fn patch(&mut self, offset: usize, bytes: &[u8]);
    fn read_back(&self, offset: usize, len: usize, buf: &mut Vec<u8>) -> Result<()>;
    fn digest(&mut self, start: usize, end: usize, f: &mut dyn FnMut(&[u8])) -> Result<()>;
}

impl DexSink for Vec<u8> {
    fn pos(&self) -> u32 {
        self.len() as u32
    }

    fn write(&mut self, bytes: &[u8]) {
        self.extend_from_slice(bytes);
    }

    fn patch(&mut self, offset: usize, bytes: &[u8]) {
        self[offset..offset + bytes.len()].copy_from_slice(bytes);
    }

    fn read_back(&self, offset: usize, len: usize, buf: &mut Vec<u8>) -> Result<()> {
        buf.clear();
        buf.extend_from_slice(&self[offset..offset + len]);
        Ok(())
    }

    fn digest(&mut self, start: usize, end: usize, f: &mut dyn FnMut(&[u8])) -> Result<()> {
        f(&self[start..end]);
        Ok(())
    }
}

const WINDOW: usize = 256 << 10;

/// Streams output through a bounded window into a temporary file. Backpatches
/// are applied before hashing or publication; I/O failures are reported at settlement.
pub struct SpoolSink {
    file: File,
    window: Vec<u8>,
    flushed: usize,
    patches: Vec<Patch>,
    patch_bytes: Vec<u8>,
    error: Option<io::Error>,
}

#[derive(Clone, Copy)]
struct Patch {
    offset: usize,
    start: usize,
    len: usize,
}

/// A serialized DEX living in an anonymous temp file.
pub struct Spooled {
    file: File,
    len: u64,
}

impl Spooled {
    /// Transfers ownership of the serialized bytes to a file-based consumer.
    /// The caller must establish its own read position.
    pub fn into_file(self) -> File {
        self.file
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Reads from the start with an independent position. Multiple readers may
    /// be interleaved or used concurrently without changing each other's position.
    pub fn reader(&self) -> impl io::Read + '_ {
        SpoolReader {
            file: &self.file,
            position: 0,
            end: self.len,
        }
    }
}

struct SpoolReader<'a> {
    file: &'a File,
    position: u64,
    end: u64,
}

impl io::Read for SpoolReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let length = buf.len().min((self.end - self.position) as usize);
        let count = self.file.read_at(&mut buf[..length], self.position)?;
        self.position += count as u64;
        Ok(count)
    }
}

impl SpoolSink {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            file: reseam_storage::temporary_file()?,
            window: Vec::with_capacity(WINDOW),
            flushed: 0,
            patches: Vec::new(),
            patch_bytes: Vec::new(),
            error: None,
        })
    }

    pub fn finish(mut self) -> Result<Spooled> {
        self.settle()?;
        Ok(Spooled {
            len: self.flushed as u64,
            file: self.file,
        })
    }

    fn flush(&mut self) {
        if self.window.is_empty() || self.error.is_some() {
            return;
        }
        if let Err(e) = self.file.write_all_at(&self.window, self.flushed as u64) {
            self.error = Some(e);
        }
        self.flushed += self.window.len();
        self.window.clear();
    }

    fn settle(&mut self) -> Result<()> {
        self.flush();
        if let Some(error) = self.error.take() {
            return Err(DexError::Io(error));
        }
        let mut window = std::mem::take(&mut self.window);
        let result = self.apply_patches(&mut window);
        window.clear();
        self.window = window;
        result.map_err(DexError::Io)?;
        self.patch_bytes.clear();
        Ok(())
    }

    fn apply_patches(&mut self, window: &mut Vec<u8>) -> io::Result<()> {
        let mut buffered = None;
        // Debug-info offsets touch every code item. Apply them in bounded pages
        // rather than issuing one file write per four-byte offset. Retaining
        // patch order also preserves the last write when patches overlap.
        for patch in self.patches.drain(..) {
            let mut offset = patch.offset;
            let mut bytes = &self.patch_bytes[patch.start..patch.start + patch.len];
            while !bytes.is_empty() {
                let start = offset / WINDOW * WINDOW;
                if buffered != Some(start) {
                    if let Some(previous) = buffered {
                        self.file.write_all_at(window, previous as u64)?;
                    }
                    window.resize(WINDOW.min(self.flushed - start), 0);
                    self.file.read_exact_at(window, start as u64)?;
                    buffered = Some(start);
                }
                let local = offset - start;
                let count = bytes.len().min(window.len() - local);
                window[local..local + count].copy_from_slice(&bytes[..count]);
                offset += count;
                bytes = &bytes[count..];
            }
        }
        if let Some(start) = buffered {
            self.file.write_all_at(window, start as u64)?;
        }
        Ok(())
    }
}

impl DexSink for SpoolSink {
    fn pos(&self) -> u32 {
        (self.flushed + self.window.len()) as u32
    }

    fn write(&mut self, bytes: &[u8]) {
        let mut remaining = bytes;
        while !remaining.is_empty() {
            let count = remaining.len().min(WINDOW - self.window.len());
            self.window.extend_from_slice(&remaining[..count]);
            remaining = &remaining[count..];
            if self.window.len() == WINDOW {
                self.flush();
            }
            if self.error.is_some() {
                self.flushed += remaining.len();
                break;
            }
        }
    }

    fn patch(&mut self, offset: usize, bytes: &[u8]) {
        if self.error.is_some() {
            return;
        }
        if offset >= self.flushed {
            let local = offset - self.flushed;
            self.window[local..local + bytes.len()].copy_from_slice(bytes);
            return;
        }
        let start = self.patch_bytes.len();
        self.patch_bytes.extend_from_slice(bytes);
        match self.patches.last_mut() {
            Some(last) if last.offset + last.len == offset && last.start + last.len == start => {
                last.len += bytes.len();
            }
            _ => self.patches.push(Patch {
                offset,
                start,
                len: bytes.len(),
            }),
        }
    }

    fn read_back(&self, offset: usize, len: usize, buf: &mut Vec<u8>) -> Result<()> {
        buf.clear();
        if offset >= self.flushed {
            let local = offset - self.flushed;
            buf.extend_from_slice(&self.window[local..local + len]);
            return Ok(());
        }
        buf.resize(len, 0);
        let in_file = len.min(self.flushed - offset);
        self.file
            .read_exact_at(&mut buf[..in_file], offset as u64)
            .map_err(DexError::Io)?;
        buf[in_file..].copy_from_slice(&self.window[..len - in_file]);
        Ok(())
    }

    fn digest(&mut self, start: usize, end: usize, f: &mut dyn FnMut(&[u8])) -> Result<()> {
        self.settle()?;
        let mut chunk = std::mem::take(&mut self.window);
        for offset in (start..end).step_by(WINDOW) {
            chunk.resize(WINDOW.min(end - offset), 0);
            self.file
                .read_exact_at(&mut chunk, offset as u64)
                .map_err(DexError::Io)?;
            f(&chunk);
        }
        chunk.clear();
        self.window = chunk;
        Ok(())
    }
}
