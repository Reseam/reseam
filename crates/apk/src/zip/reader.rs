// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use reseam_storage::file::FileReader;

use super::entry_data::EntryData;
use crate::entry::EntryName;
use crate::error::{Result, invalid};

pub(crate) type Archive = zip::ZipArchive<FileReader>;

pub(crate) fn open_archive(path: &Path) -> Result<Archive> {
    (|| Ok(zip::ZipArchive::new(FileReader::new(File::open(path)?)?)?))()
        .map_err(|error: crate::ApkError| error.in_file(path))
}

/// Entry names in archive order, as Android reads them: the stored bytes as UTF-8, whatever the
/// language encoding flag says. The ZIP reader decodes unflagged names as CP437 instead, which
/// turns any non-ASCII name into one the archive does not hold.
pub(crate) fn entry_names(archive: &Archive) -> Result<Vec<EntryName>> {
    let mut archive = archive.clone();
    (0..archive.len())
        .map(|index| {
            let entry = archive.by_index_raw(index)?;
            String::from_utf8(entry.name_raw().to_vec())
                .map(EntryName::from)
                .map_err(|_| invalid("zip entry", format!("{}: name is not UTF-8", entry.name())))
        })
        .collect()
}

/// The archive is keyed by stored name bytes, so the names [`entry_names`] reads find their entries.
pub(crate) fn contains(archive: &Archive, name: &str) -> bool {
    archive.index_for_name(name).is_some()
}

pub(crate) fn index_of(archive: &Archive, name: &str) -> Result<usize> {
    archive
        .index_for_name(name)
        .ok_or_else(|| invalid("zip entry", format!("{name}: not in the archive")))
}

/// Where `entry`'s data starts in the archive.
pub(crate) fn data_start<R: Read + ?Sized>(
    entry: &zip::read::ZipFile<'_, R>,
    name: &str,
) -> Result<u64> {
    entry
        .data_start()
        .ok_or_else(|| invalid("zip entry", format!("{name}: missing data offset")))
}

pub(crate) fn is_stored(archive: &Archive, name: &str) -> Result<bool> {
    let index = index_of(archive, name)?;
    Ok(archive.clone().by_index_raw(index)?.compression() == zip::CompressionMethod::Stored)
}

pub(crate) fn read_entry(archive: &mut Archive, name: &str) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    EntryData::open(archive, name)?.read_to_end(&mut buf)?;
    Ok(buf)
}

pub(crate) fn map_entry(archive: &mut Archive, name: &str) -> Result<reseam_storage::MappedFile> {
    if !is_stored(archive, name)? {
        let mut entry = EntryData::open(archive, name)?;
        return Ok(reseam_storage::spool_mapped(|out| {
            io::copy(&mut entry, out).map(drop)
        })?);
    }
    let file = archive.clone().into_inner();
    let entry = archive.by_index_raw(index_of(archive, name)?)?;
    let offset = data_start(&entry, name)?;
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
