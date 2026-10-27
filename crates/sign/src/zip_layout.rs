// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::ops::Range;

use crate::block::{self, BlockEnvelope};
use crate::error::{Result, invalid, io_at};
use reseam_bytes::{u16_at, u32_at};

const EOCD_SIGNATURE: &[u8; 4] = b"PK\x05\x06";
const EOCD_MIN_LEN: usize = 22;
const EOCD_CD_SIZE_FIELD: usize = 12;
const EOCD_CD_OFFSET_FIELD: Range<usize> = 16..20;

/// An APK held in a file or in memory, read by offset.
pub(crate) trait ApkSource {
    fn len(&self) -> Result<u64>;
    fn read_at(&self, offset: u64, bytes: &mut [u8]) -> Result<()>;
}

impl ApkSource for File {
    fn len(&self) -> Result<u64> {
        Ok(self
            .metadata()
            .map_err(|source| io_at("reading APK metadata", 0, source))?
            .len())
    }

    fn read_at(&self, offset: u64, bytes: &mut [u8]) -> Result<()> {
        let mut file = self;
        file.seek(SeekFrom::Start(offset))
            .and_then(|_| file.read_exact(bytes))
            .map_err(|source| io_at("reading APK", offset, source))
    }
}

impl ApkSource for [u8] {
    fn len(&self) -> Result<u64> {
        Ok(<[u8]>::len(self) as u64)
    }

    fn read_at(&self, offset: u64, bytes: &mut [u8]) -> Result<()> {
        let data = usize::try_from(offset)
            .ok()
            .and_then(|start| self.get(start..start.checked_add(bytes.len())?))
            .ok_or_else(|| invalid("apk", "read past the end of the APK"))?;
        bytes.copy_from_slice(data);
        Ok(())
    }
}

/// Where the ZIP sections of an APK lie: entry contents, the signing block
/// when there is one, the central directory, and the EOCD record.
pub(crate) struct Sections {
    /// The end of the entry contents, where any signing block starts.
    pub(crate) contents_len: u64,
    pub(crate) central_dir: Range<u64>,
    pub(crate) eocd: Vec<u8>,
}

impl Sections {
    /// Empty when the APK is unsigned or JAR-signed only.
    pub(crate) fn signing_block(&self) -> Range<u64> {
        self.contents_len..self.central_dir.start
    }
}

/// Locates the sections from the terminal ZIP32 EOCD, including its optional
/// comment, validating the signing block envelope when there is one.
pub(crate) fn sections(apk: &(impl ApkSource + ?Sized)) -> Result<Sections> {
    let len = apk.len()?;
    let tail_len = len.min((EOCD_MIN_LEN + u16::MAX as usize) as u64) as usize;
    let base = len - tail_len as u64;
    let mut tail = vec![0; tail_len];
    apk.read_at(base, &mut tail)?;
    let eocd = find_eocd(&tail)?;
    let central_dir = central_directory(&tail[eocd..], base + eocd as u64)?;
    let mut footer = vec![0; central_dir.start.min(block::FOOTER_LEN as u64) as usize];
    apk.read_at(central_dir.start - footer.len() as u64, &mut footer)?;
    let contents_len = match BlockEnvelope::from_footer(&footer, central_dir.start)? {
        Some(envelope) => {
            let mut header = [0; block::SIZE_LEN];
            apk.read_at(envelope.start, &mut header)?;
            envelope.check_header(&header)?;
            envelope.start
        }
        None => central_dir.start,
    };
    Ok(Sections {
        contents_len,
        central_dir,
        eocd: tail[eocd..].to_vec(),
    })
}

/// The offset of the EOCD record in `tail`, the end of the file.
fn find_eocd(tail: &[u8]) -> Result<usize> {
    if tail.len() < EOCD_MIN_LEN {
        return Err(invalid("apk", "file too small for ZIP"));
    }
    (0..=tail.len() - EOCD_MIN_LEN)
        .rev()
        .find(|&offset| {
            let record = &tail[offset..offset + EOCD_MIN_LEN];
            let comment_len = usize::from(u16_at(record, 20));
            &record[..4] == EOCD_SIGNATURE && offset + EOCD_MIN_LEN + comment_len == tail.len()
        })
        .ok_or_else(|| invalid("apk", "EOCD not found"))
}

/// The central directory the EOCD record at file offset `eocd_offset` names.
fn central_directory(eocd: &[u8], eocd_offset: u64) -> Result<Range<u64>> {
    let start = u64::from(u32_at(eocd, EOCD_CD_OFFSET_FIELD.start));
    let size = u64::from(u32_at(eocd, EOCD_CD_SIZE_FIELD));
    let available = eocd_offset
        .checked_sub(start)
        .ok_or_else(|| invalid("apk", "central directory offset past EOCD"))?;
    if size > available {
        return Err(invalid("apk", "central directory size extends past EOCD"));
    }
    Ok(start..eocd_offset)
}

/// `eocd` naming a central directory at `cd_offset`.
pub(crate) fn patch_cd_offset(eocd: &[u8], cd_offset: u32) -> Vec<u8> {
    let mut patched = eocd.to_vec();
    patched[EOCD_CD_OFFSET_FIELD].copy_from_slice(&cd_offset.to_le_bytes());
    patched
}

pub(crate) fn write_at(mut file: &File, offset: u64, bytes: &[u8]) -> Result<()> {
    file.seek(SeekFrom::Start(offset))
        .and_then(|_| file.write_all(bytes))
        .map_err(|source| io_at("writing signed APK", offset, source))
}

/// Moves `source` to start at `destination` within one file.
pub(crate) fn move_range(file: &File, source: Range<u64>, destination: u64) -> Result<()> {
    if source.start == destination {
        return Ok(());
    }
    let mut buffer = vec![0; 1 << 20];
    let len = source.end - source.start;
    let mut moved = 0;
    while moved < len {
        let count = (len - moved).min(buffer.len() as u64) as usize;
        // Copy in the direction that leaves overlapping source bytes intact.
        let offset = if destination > source.start {
            len - moved - count as u64
        } else {
            moved
        };
        let chunk = &mut buffer[..count];
        file.read_at(source.start + offset, chunk)?;
        write_at(file, destination + offset, chunk)?;
        moved += count as u64;
    }
    Ok(())
}
