// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::path::Path;

use reseam_storage::file::FileReader;

use crate::entry::dex_ordinal;
use crate::error::{Result, invalid};

pub(crate) type Archive = zip::ZipArchive<FileReader>;

pub(crate) fn open_archive(path: &Path) -> Result<Archive> {
    (|| Ok(zip::ZipArchive::new(FileReader::new(File::open(path)?)?)?))()
        .map_err(|error: crate::ApkError| error.in_file(path))
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
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf)?;
    Ok(buf)
}

pub(crate) fn map_entry(archive: &mut Archive, name: &str) -> Result<memmap2::Mmap> {
    let file = archive.clone().into_inner();
    let mut entry = archive.by_name(name)?;
    if entry.compression() != zip::CompressionMethod::Stored {
        return spool(&mut entry);
    }
    let offset = entry
        .data_start()
        .ok_or_else(|| invalid("zip entry", format!("{name}: missing data offset")))?;
    let size = entry.size();
    let archive_len = file.file().metadata()?.len();
    if offset.checked_add(size).is_none_or(|end| end > archive_len) {
        return Err(invalid(
            "zip entry",
            format!("{name}: stored data extends past the archive"),
        ));
    }
    let length = usize::try_from(size)
        .map_err(|_| invalid("zip entry", format!("{name}: entry is too large to map")))?;
    // SAFETY: this read-only range is inside the immutable input archive.
    Ok(unsafe {
        memmap2::MmapOptions::new()
            .offset(offset)
            .len(length)
            .map(file.file())?
    })
}

pub(crate) fn map_file(archive: &Archive) -> Result<memmap2::Mmap> {
    let file = archive.clone().into_inner();
    // SAFETY: the archive is opened read-only for the whole run and nothing
    // in this process writes to it.
    Ok(unsafe { memmap2::Mmap::map(file.file())? })
}

pub(crate) fn spool(reader: &mut impl Read) -> Result<memmap2::Mmap> {
    map_spooled(&spool_file(reader)?)
}

pub(crate) fn spool_file(reader: &mut impl Read) -> Result<File> {
    let mut file = tempfile::tempfile()?;
    let mut out = BufWriter::with_capacity(1 << 20, &mut file);
    io::copy(reader, &mut out)?;
    out.flush()?;
    drop(out);
    Ok(file)
}

pub(crate) fn map_spooled(file: &File) -> Result<memmap2::Mmap> {
    // SAFETY: spooled files are immutable after creation and never exposed for writing.
    Ok(unsafe { memmap2::MmapOptions::new().map(file)? })
}

pub(crate) fn copy_spooled(file: &File, output: &mut impl Write) -> Result<()> {
    let mut file = io::BufReader::new(FileReader::new(file.try_clone()?)?);
    io::copy(&mut file, output)?;
    Ok(())
}
