// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::dex::dex_sort_key;
use crate::error::Result;
use std::fs::File;
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::FileExt;
use std::path::Path;
use std::sync::Arc;

/// Wrapper around ZipArchive providing APK-oriented access.
pub struct ApkReader<R: Read + Seek> {
    archive: zip::ZipArchive<R>,
}

impl<R: Read + Seek> ApkReader<R> {
    /// Open a ZIP archive for reading.
    pub fn new(reader: R) -> Result<Self> {
        let archive = zip::ZipArchive::new(reader)?;
        Ok(Self { archive })
    }

    /// Number of entries in the archive.
    pub fn len(&self) -> usize {
        self.archive.len()
    }

    pub fn is_empty(&self) -> bool {
        self.archive.is_empty()
    }

    /// List all entry names in the archive.
    pub fn entry_names(&mut self) -> Vec<String> {
        (0..self.archive.len())
            .filter_map(|i| {
                self.archive
                    .by_index_raw(i)
                    .ok()
                    .map(|e| e.name().to_string())
            })
            .collect()
    }

    /// Read a single entry's decompressed contents by name.
    pub fn read_entry(&mut self, name: &str) -> Result<Vec<u8>> {
        let mut entry = self.archive.by_name(name)?;
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut buf)?;
        Ok(buf)
    }

    /// Check if an entry exists.
    pub fn contains(&self, name: &str) -> bool {
        // ZipArchive doesn't have a contains method, but we can check via index_for_name
        self.archive.index_for_name(name).is_some()
    }

    /// Get the list of DEX entry names (classes.dex, classes2.dex, ...) in sorted order.
    pub fn dex_entry_names(&mut self) -> Vec<String> {
        let mut names: Vec<String> = self
            .entry_names()
            .into_iter()
            .filter(|name| is_dex_entry(name))
            .collect();
        names.sort_by_key(|n| dex_sort_key(n));
        names
    }

    /// Read AndroidManifest.xml as raw bytes (binary XML format).
    pub fn read_manifest(&mut self) -> Result<Vec<u8>> {
        self.read_entry("AndroidManifest.xml")
    }

    /// Consume self and return the inner ZipArchive (needed by writer for pass-through).
    pub fn into_archive(self) -> zip::ZipArchive<R> {
        self.archive
    }

    /// Borrow the inner ZipArchive.
    pub fn archive(&self) -> &zip::ZipArchive<R> {
        &self.archive
    }

    /// Mutably borrow the inner ZipArchive.
    pub fn archive_mut(&mut self) -> &mut zip::ZipArchive<R> {
        &mut self.archive
    }
}

/// Returns true if the entry name matches `classes.dex` or `classesN.dex`.
fn is_dex_entry(name: &str) -> bool {
    name == "classes.dex" || (name.starts_with("classes") && name.ends_with(".dex"))
}

/// A positional file reader: clones share the descriptor but keep their own
/// offset, so one parsed archive can be read from several threads at once.
#[derive(Clone)]
pub struct SharedFile {
    file: Arc<File>,
    pos: u64,
    len: u64,
}

impl SharedFile {
    pub fn file(&self) -> &File {
        &self.file
    }

    pub fn open(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        let len = file.metadata()?.len();
        Ok(Self {
            file: Arc::new(file),
            pos: 0,
            len,
        })
    }
}

impl Read for SharedFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.file.read_at(buf, self.pos)?;
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for SharedFile {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let (base, delta) = match pos {
            SeekFrom::Start(offset) => (offset, 0),
            SeekFrom::End(delta) => (self.len, delta),
            SeekFrom::Current(delta) => (self.pos, delta),
        };
        self.pos = base
            .checked_add_signed(delta)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek before start"))?;
        Ok(self.pos)
    }
}

/// The bytes of an entry as a file-backed mapping: a stored entry is mapped
/// straight from the archive, a deflated one is inflated into an anonymous
/// temp file first. Either way the pages are reclaimable, never heap.
pub fn entry_bytes(archive: &File, entry: &mut zip::read::ZipFile<'_>) -> Result<memmap2::Mmap> {
    if entry.compression() == zip::CompressionMethod::Stored {
        // SAFETY: the archive is opened read-only for the whole run and
        // nothing in this process writes to it.
        let mapped = unsafe {
            memmap2::MmapOptions::new()
                .offset(entry.data_start())
                .len(entry.size() as usize)
                .map(archive)?
        };
        return Ok(mapped);
    }
    spool_entry(entry)
}

/// Inflates an entry into an anonymous temp file and maps it, so the bytes are
/// file-backed pages the kernel can reclaim instead of anonymous memory.
pub fn spool_entry(entry: &mut impl Read) -> Result<memmap2::Mmap> {
    let mut file = tempfile::tempfile()?;
    let mut out = BufWriter::with_capacity(1 << 20, &mut file);
    io::copy(entry, &mut out)?;
    out.flush()?;
    drop(out);
    // SAFETY: the file is unlinked and reachable only through this handle, so
    // nothing can change its contents while the mapping is alive.
    Ok(unsafe { memmap2::MmapOptions::new().map(&file)? })
}
