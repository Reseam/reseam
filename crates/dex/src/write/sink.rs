// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::File;
use std::io;

use crate::error::{DexError, Result};
use crate::util::file::FileExt;

/// Destination of a DEX serialization. The writer appends sequentially and
/// backpatches tables it emitted earlier; how those land is up to the sink.
pub trait DexSink {
    fn pos(&self) -> u32;
    fn write(&mut self, bytes: &[u8]);
    fn patch(&mut self, offset: usize, bytes: &[u8]);
    /// Copies `len` bytes written at `offset` into `buf`, ignoring patches
    /// still pending on them.
    fn read_back(&self, offset: usize, len: usize, buf: &mut Vec<u8>) -> Result<()>;
    /// Feeds `start..end` of the output, with every patch applied, to `f`.
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

/// Streams the output into an anonymous temp file through a bounded window.
/// Patches behind the window are queued; when the body is complete the file
/// is mapped once, the queue is applied to the mapping, and later patches
/// and digests go straight through it, so the serialized DEX never has to
/// sit on the heap and scattered backpatches cost no system calls.
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
    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The spooled bytes, read from the start.
    pub fn reader(&self) -> io::Result<impl io::Read + '_> {
        let mut file = &self.file;
        io::Seek::seek(&mut file, io::SeekFrom::Start(0))?;
        Ok(io::Read::take(file, self.len))
    }
}

impl SpoolSink {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            file: tempfile::tempfile()?,
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

    /// Flushes the body, maps the file, and applies every queued patch.
    /// Flushes the window and writes every pending patch into the file.
    fn settle(&mut self) -> Result<()> {
        self.flush();
        if let Some(error) = self.error.take() {
            return Err(DexError::Io(error));
        }
        for patch in self.patches.drain(..) {
            self.file
                .write_all_at(
                    &self.patch_bytes[patch.start..patch.start + patch.len],
                    patch.offset as u64,
                )
                .map_err(DexError::Io)?;
        }
        self.patch_bytes.clear();
        Ok(())
    }
}

impl DexSink for SpoolSink {
    fn pos(&self) -> u32 {
        (self.flushed + self.window.len()) as u32
    }

    fn write(&mut self, bytes: &[u8]) {
        self.window.extend_from_slice(bytes);
        if self.window.len() >= WINDOW {
            self.flush();
        }
    }

    fn patch(&mut self, offset: usize, bytes: &[u8]) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn read_all(spooled: &Spooled) -> Vec<u8> {
        let mut bytes = Vec::new();
        io::Read::read_to_end(&mut spooled.reader().unwrap(), &mut bytes).unwrap();
        bytes
    }

    #[test]
    fn spool_matches_vec_with_patches_across_the_window() {
        let mut expected = Vec::new();
        let mut spool = SpoolSink::new().unwrap();
        let body: Vec<u8> = (0..(3 * WINDOW + 17)).map(|i| (i % 251) as u8).collect();
        for sink in [&mut expected as &mut dyn DexSink, &mut spool] {
            sink.write(&[0; 16]);
            sink.write(&body);
            sink.patch(0, &[1, 2, 3, 4]);
            sink.patch(12, &[9; 4]);
            sink.patch(WINDOW - 2, &[7; 4]);
            sink.patch(2 * WINDOW + 8, &[5; 4]);
            sink.patch(sink.pos() as usize - 4, &[8; 4]);
        }
        let mut hashed = Vec::new();
        spool
            .digest(4, 2 * WINDOW + 20, &mut |c| hashed.extend_from_slice(c))
            .unwrap();
        assert_eq!(hashed, expected[4..2 * WINDOW + 20]);
        let mut back = Vec::new();
        spool.read_back(24, 16, &mut back).unwrap();
        assert_eq!(back, body[8..24]);
        spool
            .read_back(spool.pos() as usize - 12, 6, &mut back)
            .unwrap();
        assert_eq!(back, expected[expected.len() - 12..expected.len() - 6]);
        spool.patch(4, &[6; 4]);
        expected.patch(4, &[6; 4]);
        let spooled = spool.finish().unwrap();
        assert_eq!(spooled.len(), expected.len() as u64);
        assert_eq!(read_all(&spooled), expected);
    }
}
