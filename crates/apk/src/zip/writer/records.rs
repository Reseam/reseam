// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! ZIP records as APK writers emit them: no ZIP64, no data descriptors.

use crate::error::{Result, invalid};
use crate::zip::Compression;

const LOCAL_HEADER: u32 = 0x0403_4b50;
const CENTRAL_HEADER: u32 = 0x0201_4b50;
const END_OF_CENTRAL_DIRECTORY: u32 = 0x0605_4b50;
/// APPNOTE 4.6.11 data stream alignment: a 2-byte alignment, then padding.
const ALIGNMENT_FIELD: u16 = 0xa11e;
const UTF8_NAME: u16 = 1 << 11;
/// Unix, ZIP 2.0.
const MADE_BY_UNIX: u16 = 0x0314;
const REGULAR_FILE: u32 = 0o100_000;
const DEFAULT_MODE: u32 = 0o644;
pub(super) const LOCAL_HEADER_LEN: u64 = 30;

impl Compression {
    pub(super) fn of(method: zip::CompressionMethod) -> Result<Self> {
        match method {
            zip::CompressionMethod::Stored => Ok(Self::Stored),
            zip::CompressionMethod::Deflated => Ok(Self::Deflated),
            other => Err(invalid(
                "zip entry",
                format!("unsupported compression method {other}"),
            )),
        }
    }

    fn code(self) -> u16 {
        match self {
            Self::Stored => 0,
            Self::Deflated => 8,
        }
    }

    fn version_needed(self) -> u16 {
        match self {
            Self::Stored => 10,
            Self::Deflated => 20,
        }
    }
}

/// An MS-DOS timestamp. Entries the writer creates carry the earliest one,
/// so output does not depend on when it was written.
#[derive(Debug, Clone, Copy)]
pub(super) struct DosTime {
    pub time: u16,
    pub date: u16,
}

impl DosTime {
    pub(super) const EPOCH: Self = Self {
        time: 0,
        date: 1 << 5 | 1,
    };
}

/// The fields a local header and its central directory record share.
#[derive(Debug, Clone, Copy)]
pub(super) struct EntryHeader {
    pub method: Compression,
    pub modified: DosTime,
    pub crc: u32,
    pub compressed: u32,
    pub size: u32,
    pub unix_mode: u32,
}

impl EntryHeader {
    /// A new regular file, timestamped with [`DosTime::EPOCH`].
    pub(super) fn new(method: Compression, crc: u32, compressed: u32, size: u32) -> Self {
        Self {
            method,
            modified: DosTime::EPOCH,
            crc,
            compressed,
            size,
            unix_mode: DEFAULT_MODE | REGULAR_FILE,
        }
    }

    /// The header of an entry copied as an archive holds it.
    pub(super) fn of(entry: &zip::read::ZipFile<'_, impl std::io::Read>) -> Result<Self> {
        let (compressed, size) = Self::sizes(entry.compressed_size(), entry.size())?;
        Ok(Self {
            method: Compression::of(entry.compression())?,
            modified: entry
                .last_modified()
                .map_or(DosTime::EPOCH, |time| DosTime {
                    time: time.timepart(),
                    date: time.datepart(),
                }),
            crc: entry.crc32(),
            compressed,
            size,
            unix_mode: entry.unix_mode().unwrap_or(DEFAULT_MODE) | REGULAR_FILE,
        })
    }

    pub(super) fn sizes(compressed: u64, size: u64) -> Result<(u32, u32)> {
        Ok((zip32(compressed, "entry size")?, zip32(size, "entry size")?))
    }

    fn flags(name: &str) -> u16 {
        if name.is_ascii() { 0 } else { UTF8_NAME }
    }

    /// The local header of an entry named `name` starting at `offset`, padded
    /// so its data starts on a multiple of `alignment`.
    pub(super) fn local(&self, name: &str, offset: u64, alignment: u16, out: &mut Vec<u8>) {
        out.clear();
        put32(out, LOCAL_HEADER);
        put16(out, self.method.version_needed());
        put16(out, Self::flags(name));
        self.put_fields(out);
        put16(out, name.len() as u16);
        let data = offset + LOCAL_HEADER_LEN + name.len() as u64;
        let padding = alignment_padding(data, alignment);
        put16(out, padding.len() as u16);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&padding);
    }

    pub(super) fn central(&self, name: &str, offset: u32, out: &mut Vec<u8>) {
        put32(out, CENTRAL_HEADER);
        put16(out, MADE_BY_UNIX);
        put16(out, self.method.version_needed());
        put16(out, Self::flags(name));
        self.put_fields(out);
        put16(out, name.len() as u16);
        put16(out, 0);
        put16(out, 0);
        put16(out, 0);
        put16(out, 0);
        put32(out, self.unix_mode << 16);
        put32(out, offset);
        out.extend_from_slice(name.as_bytes());
    }

    /// Method, time, CRC and sizes, at offset 8 of a local header.
    fn put_fields(&self, out: &mut Vec<u8>) {
        put16(out, self.method.code());
        put16(out, self.modified.time);
        put16(out, self.modified.date);
        self.put_checksum(out);
    }

    /// CRC and sizes, which a header patched after its data was written
    /// receives at [`Self::CHECKSUM_OFFSET`].
    pub(super) fn put_checksum(&self, out: &mut Vec<u8>) {
        put32(out, self.crc);
        put32(out, self.compressed);
        put32(out, self.size);
    }

    pub(super) const CHECKSUM_OFFSET: u64 = 14;
}

pub(super) fn end_of_central_directory(
    entries: usize,
    size: u64,
    offset: u64,
    out: &mut Vec<u8>,
) -> Result<()> {
    let entries = u16::try_from(entries)
        .map_err(|_| invalid("apk output", "more entries than a ZIP without ZIP64 holds"))?;
    put32(out, END_OF_CENTRAL_DIRECTORY);
    put16(out, 0);
    put16(out, 0);
    put16(out, entries);
    put16(out, entries);
    put32(out, zip32(size, "central directory size")?);
    put32(out, zip32(offset, "central directory offset")?);
    put16(out, 0);
    Ok(())
}

pub(super) fn zip32(value: u64, what: &str) -> Result<u32> {
    u32::try_from(value)
        .ok()
        .filter(|&value| value != u32::MAX)
        .ok_or_else(|| invalid("apk output", format!("{what} needs ZIP64")))
}

/// The extra field that moves data at `data` to the next multiple of
/// `alignment`; empty when it is already aligned.
fn alignment_padding(data: u64, alignment: u16) -> Vec<u8> {
    let alignment = u64::from(alignment.max(1));
    let misaligned = data % alignment;
    if misaligned == 0 {
        return Vec::new();
    }
    let mut length = alignment - misaligned;
    while length < 6 {
        length += alignment;
    }
    let mut field = Vec::with_capacity(length as usize);
    put16(&mut field, ALIGNMENT_FIELD);
    put16(&mut field, (length - 4) as u16);
    put16(&mut field, alignment as u16);
    field.resize(length as usize, 0);
    field
}

fn put16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
