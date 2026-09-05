// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::{invalid, Result};

const APK_SIG_BLOCK_MAGIC: &[u8; 16] = b"APK Sig Block 42";
const EOCD_SIGNATURE: &[u8; 4] = b"PK\x05\x06";
const EOCD_MIN_LEN: usize = 22;
const EOCD_CD_OFFSET_FIELD: std::ops::Range<usize> = 16..20;
const PAIR_OVERHEAD: usize = 12;
const BLOCK_OVERHEAD: usize = 32;
const BLOCK_ID_PADDING: u32 = 0x4272_6577;

pub const BLOCK_ID_V2: u32 = 0x7109_871a;

pub struct ApkSections<'a> {
    pub contents: &'a [u8],
    pub central_dir: &'a [u8],
    pub eocd: &'a [u8],
}

pub struct Eocd {
    pub offset: usize,
    pub cd_offset: u32,
    pub cd_size: u32,
}

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
            cd_offset: le_u32(record, 16),
            cd_size: le_u32(record, 12),
        });
    }
    Err(invalid("apk", "EOCD not found"))
}

/// Splits an APK into its ZIP sections, leaving any existing signing block out.
pub fn split_apk(data: &[u8]) -> Result<ApkSections<'_>> {
    let eocd = find_eocd(data)?;
    let cd_offset = eocd.cd_offset as usize;
    if cd_offset > eocd.offset {
        return Err(invalid("apk", "central directory offset past EOCD"));
    }
    let contents_end = signing_block_start(data, cd_offset).unwrap_or(cd_offset);
    Ok(ApkSections {
        contents: &data[..contents_end],
        central_dir: &data[cd_offset..eocd.offset],
        eocd: &data[eocd.offset..],
    })
}

fn signing_block_start(data: &[u8], cd_offset: usize) -> Option<usize> {
    let magic_start = cd_offset.checked_sub(APK_SIG_BLOCK_MAGIC.len())?;
    if data.get(magic_start..cd_offset)? != APK_SIG_BLOCK_MAGIC {
        return None;
    }
    let block_size = le_u64_at(data, magic_start.checked_sub(8)?)? as usize;
    let block_start = cd_offset.checked_sub(8 + block_size)?;
    let leading_size = le_u64_at(data, block_start)? as usize;
    (leading_size == block_size).then_some(block_start)
}

pub(crate) fn signing_block_len(value_lens: impl IntoIterator<Item = usize>) -> usize {
    BLOCK_OVERHEAD
        + value_lens
            .into_iter()
            .map(|len| PAIR_OVERHEAD + len)
            .sum::<usize>()
}

/// Builds a block of exactly `target_len` bytes; the slack goes into a padding pair.
pub(crate) fn build_signing_block(pairs: &[(u32, Vec<u8>)], target_len: usize) -> Result<Vec<u8>> {
    let len = signing_block_len(pairs.iter().map(|(_, value)| value.len()));
    let padding = match target_len.checked_sub(len) {
        Some(0) => None,
        Some(slack) if slack >= PAIR_OVERHEAD => Some(slack - PAIR_OVERHEAD),
        _ => {
            return Err(invalid(
                "signing block",
                format!("block of {len} bytes does not fit target length {target_len}"),
            ))
        }
    };
    let block_size = (target_len - 8) as u64;
    let mut block = Vec::with_capacity(target_len);
    block.extend_from_slice(&block_size.to_le_bytes());
    for (id, value) in pairs {
        write_pair_header(&mut block, *id, value.len());
        block.extend_from_slice(value);
    }
    if let Some(padding) = padding {
        write_pair_header(&mut block, BLOCK_ID_PADDING, padding);
        block.resize(block.len() + padding, 0);
    }
    block.extend_from_slice(&block_size.to_le_bytes());
    block.extend_from_slice(APK_SIG_BLOCK_MAGIC);
    debug_assert_eq!(block.len(), target_len);
    Ok(block)
}

pub(crate) fn patch_cd_offset(eocd: &[u8], cd_offset: u32) -> Vec<u8> {
    let mut patched = eocd.to_vec();
    patched[EOCD_CD_OFFSET_FIELD].copy_from_slice(&cd_offset.to_le_bytes());
    patched
}

fn write_pair_header(block: &mut Vec<u8>, id: u32, value_len: usize) {
    block.extend_from_slice(&((4 + value_len) as u64).to_le_bytes());
    block.extend_from_slice(&id.to_le_bytes());
}

fn le_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}

fn le_u64_at(data: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        data.get(offset..offset + 8)?.try_into().ok()?,
    ))
}
