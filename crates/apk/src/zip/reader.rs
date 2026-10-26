// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs::File;
use std::io::{self, Read};
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

pub(crate) fn is_stored(archive: &Archive, name: &str) -> Result<bool> {
    let index = archive
        .index_for_name(name)
        .ok_or_else(|| invalid("zip entry", format!("{name}: not in the archive")))?;
    Ok(archive.clone().by_index_raw(index)?.compression() == zip::CompressionMethod::Stored)
}

pub(crate) fn read_entry(archive: &mut Archive, name: &str) -> Result<Vec<u8>> {
    let mut entry = archive.by_name(name)?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf)?;
    Ok(buf)
}

pub(crate) fn map_entry(archive: &mut Archive, name: &str) -> Result<reseam_storage::MappedFile> {
    let file = archive.clone().into_inner();
    let mut entry = archive.by_name(name)?;
    if entry.compression() != zip::CompressionMethod::Stored {
        return Ok(reseam_storage::spool_mapped(|out| {
            io::copy(&mut entry, out).map(drop)
        })?);
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
    Ok(unsafe { reseam_storage::map_range(file.file(), offset, length)? })
}

pub(crate) fn map_file(archive: &Archive) -> Result<reseam_storage::MappedFile> {
    let file = archive.clone().into_inner();
    // SAFETY: the archive is opened read-only for the whole run and nothing
    // in this process writes to it.
    Ok(unsafe { reseam_storage::map_file(file.file())? })
}
