// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::ops::Range;

use crate::error::{Result, SignError, invalid, io_at};

pub(crate) const APK_SIG_BLOCK_MAGIC: &[u8; 16] = b"APK Sig Block 42";
const EOCD_SIGNATURE: &[u8; 4] = b"PK\x05\x06";
const EOCD_MIN_LEN: usize = 22;
pub(crate) const EOCD_CD_OFFSET_FIELD: Range<usize> = 16..20;
pub(crate) const BLOCK_OVERHEAD: usize = 32;

pub const BLOCK_ID_V2: u32 = 0x7109_871a;
pub const BLOCK_ID_V3: u32 = 0xf053_68c0;

pub struct ApkSections<'a> {
    pub(crate) contents: &'a [u8],
    pub(crate) central_dir: &'a [u8],
    pub(crate) eocd: &'a [u8],
}

pub struct Eocd {
    pub(crate) offset: usize,
    pub(crate) cd_offset: u32,
    pub(crate) cd_size: u32,
}

impl<'a> ApkSections<'a> {
    pub fn contents(&self) -> &'a [u8] {
        self.contents
    }
    pub fn central_directory(&self) -> &'a [u8] {
        self.central_dir
    }
    pub fn eocd(&self) -> &'a [u8] {
        self.eocd
    }
}

impl Eocd {
    pub fn offset(&self) -> usize {
        self.offset
    }
    pub fn central_directory_offset(&self) -> u32 {
        self.cd_offset
    }
    pub fn central_directory_size(&self) -> u32 {
        self.cd_size
    }

    pub(crate) fn central_directory(&self, base: u64) -> Result<Range<u64>> {
        let start = u64::from(self.cd_offset);
        let end = base + self.offset as u64;
        let available = end
            .checked_sub(start)
            .ok_or_else(|| invalid("apk", "central directory offset past EOCD"))?;
        if u64::from(self.cd_size) > available {
            return Err(invalid("apk", "central directory size extends past EOCD"));
        }
        Ok(start..end)
    }
}

/// Locates the terminal ZIP32 EOCD, including its optional comment.
/// The returned offsets describe the record; section bounds are checked by `split_apk`.
pub fn find_eocd(data: &[u8]) -> Result<Eocd> {
    if data.len() < EOCD_MIN_LEN {
        return Err(invalid("apk", "file too small for ZIP"));
    }
    let search_start = data.len().saturating_sub(EOCD_MIN_LEN + u16::MAX as usize);
    for offset in (search_start..=data.len() - EOCD_MIN_LEN).rev() {
        let record = &data[offset..offset + EOCD_MIN_LEN];
        if &record[..4] != EOCD_SIGNATURE {
            continue;
        }
        let comment_len = u16::from_le_bytes([record[20], record[21]]) as usize;
        if offset + EOCD_MIN_LEN + comment_len != data.len() {
            continue;
        }
        return Ok(Eocd {
            offset,
            cd_offset: le_u32(record, EOCD_CD_OFFSET_FIELD.start),
            cd_size: le_u32(record, 12),
        });
    }
    Err(invalid("apk", "EOCD not found"))
}

/// Splits an APK into its ZIP sections, leaving any existing signing block out.
pub fn split_apk(data: &[u8]) -> Result<ApkSections<'_>> {
    let eocd = find_eocd(data)?;
    let central_dir = eocd.central_directory(0)?;
    let cd_offset = central_dir.start as usize;
    let contents_end = signing_block_start(data, cd_offset)?.unwrap_or(cd_offset);
    Ok(ApkSections {
        contents: &data[..contents_end],
        central_dir: &data[central_dir.start as usize..central_dir.end as usize],
        eocd: &data[eocd.offset..],
    })
}

/// The APK Signing Block, when the archive carries one. The slice runs from
/// the block's leading size field through its trailing magic, ready for
/// [`find_pair`]; `None` means the archive is unsigned or JAR-signed only.
pub fn block(data: &[u8]) -> Result<Option<&[u8]>> {
    let cd_offset = find_eocd(data)?.central_directory(0)?.start as usize;
    Ok(signing_block_start(data, cd_offset)?.map(|start| &data[start..cd_offset]))
}

/// Finds the first pair with `id`, returning absence only for a missing ID.
/// Invalid block sizes or pair lengths encountered before the match are errors.
/// As on Android, pairs following the first match are not interpreted.
pub fn find_pair(block: &[u8], id: u32) -> Result<Option<&[u8]>> {
    if signing_block_start(block, block.len())? != Some(0) {
        return Err(invalid("signing block", "invalid block envelope"));
    }
    let mut pairs = Reader::new(&block[8..block.len() - 24], "signing block pairs");
    while !pairs.is_empty() {
        let len = usize::try_from(pairs.u64()?)
            .map_err(|_| invalid("signing block", "pair length exceeds address space"))?;
        if len < 4 {
            return Err(invalid("signing block", "pair length excludes its ID"));
        }
        let mut pair = Reader::new(pairs.take(len)?, "signing pair");
        if pair.u32()? == id {
            return Ok(Some(pair.take(len - 4)?));
        }
    }
    Ok(None)
}

struct BlockEnvelope {
    start: u64,
    size: u64,
}

impl BlockEnvelope {
    fn from_footer(footer: &[u8], cd_offset: u64) -> Result<Option<Self>> {
        if !footer.ends_with(APK_SIG_BLOCK_MAGIC) {
            return Ok(None);
        }
        let size_end = footer
            .len()
            .checked_sub(APK_SIG_BLOCK_MAGIC.len())
            .ok_or_else(|| invalid("signing block", "missing magic"))?;
        let size_start = size_end
            .checked_sub(8)
            .ok_or_else(|| invalid("signing block", "missing footer size"))?;
        let size = Reader::new(&footer[size_start..size_end], "signing block footer").u64()?;
        let len = size
            .checked_add(8)
            .filter(|len| *len >= BLOCK_OVERHEAD as u64)
            .ok_or_else(|| invalid("signing block", "invalid footer size"))?;
        let start = cd_offset
            .checked_sub(len)
            .ok_or_else(|| invalid("signing block", "footer size extends before file start"))?;
        Ok(Some(Self { start, size }))
    }

    fn check_header(&self, header: &[u8]) -> Result<()> {
        if Reader::new(header, "signing block header").u64()? != self.size {
            return Err(invalid("signing block", "header and footer sizes differ"));
        }
        Ok(())
    }
}

fn signing_block_start(data: &[u8], cd_offset: usize) -> Result<Option<usize>> {
    let prefix = data
        .get(..cd_offset)
        .ok_or_else(|| invalid("apk", "central directory offset past end"))?;
    let footer = &prefix[prefix.len().saturating_sub(24)..];
    let Some(envelope) = BlockEnvelope::from_footer(footer, cd_offset as u64)? else {
        return Ok(None);
    };
    let start = envelope.start as usize;
    envelope.check_header(&data[start..start + 8])?;
    Ok(Some(start))
}

pub(crate) struct FileSections {
    pub(crate) contents_len: u64,
    pub(crate) central_dir: Range<u64>,
    pub(crate) eocd: Vec<u8>,
}

pub(crate) fn file_sections(file: &File) -> Result<FileSections> {
    let len = file
        .metadata()
        .map_err(|source| io_at("reading APK metadata", 0, source))?
        .len();
    let tail_len = len.min((EOCD_MIN_LEN + u16::MAX as usize) as u64) as usize;
    let base = len - tail_len as u64;
    let mut tail = vec![0; tail_len];
    read_at(file, base, &mut tail)?;
    let eocd = find_eocd(&tail)?;
    let central_dir = eocd.central_directory(base)?;
    let mut footer = vec![0; central_dir.start.min(24) as usize];
    read_at(file, central_dir.start - footer.len() as u64, &mut footer)?;
    let contents_len = match BlockEnvelope::from_footer(&footer, central_dir.start)? {
        Some(envelope) => {
            let mut header = [0; 8];
            read_at(file, envelope.start, &mut header)?;
            envelope.check_header(&header)?;
            envelope.start
        }
        None => central_dir.start,
    };
    Ok(FileSections {
        contents_len,
        central_dir,
        eocd: tail[eocd.offset..].to_vec(),
    })
}

fn le_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        data[offset..offset + 4]
            .try_into()
            .expect("ZIP field has four bytes"),
    )
}

pub(crate) struct Reader<'a> {
    data: &'a [u8],
    offset: usize,
    section: &'static str,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(data: &'a [u8], section: &'static str) -> Self {
        Self {
            data,
            offset: 0,
            section,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub(crate) fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let Some((value, remaining)) = self.data.split_at_checked(len) else {
            return Err(self.error("field extends past end"));
        };
        self.data = remaining;
        self.offset += len;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut bytes = [0; N];
        bytes.copy_from_slice(self.take(N)?);
        Ok(bytes)
    }

    pub(crate) fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array::<4>()?))
    }

    pub(crate) fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.array::<8>()?))
    }

    pub(crate) fn prefixed(&mut self) -> Result<&'a [u8]> {
        let len = self.u32()? as usize;
        self.take(len)
    }

    fn error(&self, reason: &'static str) -> SignError {
        SignError::Malformed {
            section: self.section,
            offset: self.offset,
            reason,
        }
    }
}

pub(crate) fn read_at(mut file: &File, offset: u64, bytes: &mut [u8]) -> Result<()> {
    file.seek(SeekFrom::Start(offset))
        .and_then(|_| file.read_exact(bytes))
        .map_err(|source| io_at("reading APK", offset, source))
}
