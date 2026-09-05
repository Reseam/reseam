// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::File;
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::FileExt;
use std::path::Path;
use std::sync::Arc;

use crate::entry::dex_ordinal;
use crate::error::Result;

/// A parsed archive whose clones share one descriptor and one central
/// directory, so entries can be read from several threads at once.
pub(crate) type Archive = zip::ZipArchive<SharedFile>;

pub(crate) fn open_archive(path: &Path) -> Result<Archive> {
    Ok(zip::ZipArchive::new(SharedFile::open(path)?)?)
}

pub(crate) fn entry_names(archive: &Archive) -> Vec<String> {
    archive.file_names().map(String::from).collect()
}

pub(crate) fn dex_entry_names(archive: &Archive) -> Vec<String> {
    let mut names: Vec<(u32, String)> = archive
        .file_names()
        .filter_map(|name| Some((dex_ordinal(name)?, name.into())))
        .collect();
    names.sort_unstable_by_key(|(ordinal, _)| *ordinal);
    names.into_iter().map(|(_, name)| name).collect()
}

pub(crate) fn contains(archive: &Archive, name: &str) -> bool {
    archive.index_for_name(name).is_some()
}

pub(crate) fn read_entry(archive: &mut Archive, name: &str) -> Result<Vec<u8>> {
    let mut entry = archive.by_name(name)?;
    let mut buf = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut buf)?;
    Ok(buf)
}

/// The bytes of an entry as a file-backed mapping: a stored entry is mapped
/// straight from the archive, a deflated one is inflated into an anonymous
/// temp file first. Either way the pages are reclaimable, never heap.
pub(crate) fn map_entry(archive: &mut Archive, name: &str) -> Result<memmap2::Mmap> {
    let file = archive.clone().into_inner();
    let mut entry = archive.by_name(name)?;
    if entry.compression() != zip::CompressionMethod::Stored {
        return spool(&mut entry);
    }
    // SAFETY: the archive is opened read-only for the whole run and nothing
    // in this process writes to it.
    Ok(unsafe {
        memmap2::MmapOptions::new()
            .offset(entry.data_start())
            .len(entry.size() as usize)
            .map(file.file())?
    })
}

pub(crate) fn spool(reader: &mut impl Read) -> Result<memmap2::Mmap> {
    let mut file = tempfile::tempfile()?;
    let mut out = BufWriter::with_capacity(1 << 20, &mut file);
    io::copy(reader, &mut out)?;
    out.flush()?;
    drop(out);
    // SAFETY: the file is unlinked and reachable only through this handle, so
    // nothing can change its contents while the mapping is alive.
    Ok(unsafe { memmap2::MmapOptions::new().map(&file)? })
}

/// A positional file reader: clones share the descriptor but keep their own
/// offset.
#[derive(Clone)]
pub(crate) struct SharedFile {
    file: Arc<File>,
    pos: u64,
    len: u64,
}

impl SharedFile {
    fn open(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        let len = file.metadata()?.len();
        Ok(Self {
            file: Arc::new(file),
            pos: 0,
            len,
        })
    }

    fn file(&self) -> &File {
        &self.file
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
