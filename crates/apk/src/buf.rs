// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::{truncated, Result};

pub(crate) fn require_len(
    buf: &[u8],
    offset: usize,
    needed: usize,
    section: &'static str,
) -> Result<()> {
    if offset.checked_add(needed).is_none_or(|end| end > buf.len()) {
        return Err(truncated(
            section,
            offset,
            needed,
            buf.len().saturating_sub(offset),
        ));
    }
    Ok(())
}

pub(crate) fn slice<'a>(
    buf: &'a [u8],
    offset: usize,
    len: usize,
    section: &'static str,
) -> Result<&'a [u8]> {
    require_len(buf, offset, len, section)?;
    Ok(&buf[offset..offset + len])
}

pub(crate) fn read_u8(buf: &[u8], offset: usize, section: &'static str) -> Result<u8> {
    require_len(buf, offset, 1, section)?;
    Ok(buf[offset])
}

pub(crate) fn read_u16_le(buf: &[u8], offset: usize, section: &'static str) -> Result<u16> {
    let bytes = slice(buf, offset, 2, section)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

pub(crate) fn read_u32_le(buf: &[u8], offset: usize, section: &'static str) -> Result<u32> {
    let bytes = slice(buf, offset, 4, section)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

#[inline]
pub(crate) fn write_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

#[inline]
pub(crate) fn write_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}
