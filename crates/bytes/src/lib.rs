// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Little-endian fields of binary formats: bounds-checked reads and appends.

/// A read that runs past the end of its buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("truncated {section} at offset {offset:#x}: need {needed} bytes, have {available}")]
pub struct Truncated {
    pub section: &'static str,
    pub offset: usize,
    pub needed: usize,
    pub available: usize,
}

pub type Result<T> = std::result::Result<T, Truncated>;

#[inline]
pub fn require_len(buf: &[u8], offset: usize, needed: usize, section: &'static str) -> Result<()> {
    if offset.checked_add(needed).is_none_or(|end| end > buf.len()) {
        return Err(Truncated {
            section,
            offset,
            needed,
            available: buf.len().saturating_sub(offset),
        });
    }
    Ok(())
}

#[inline]
pub fn slice<'a>(
    buf: &'a [u8],
    offset: usize,
    len: usize,
    section: &'static str,
) -> Result<&'a [u8]> {
    require_len(buf, offset, len, section)?;
    Ok(&buf[offset..offset + len])
}

#[inline]
pub fn read_u8(buf: &[u8], offset: usize, section: &'static str) -> Result<u8> {
    Ok(slice(buf, offset, 1, section)?[0])
}

#[inline]
pub fn read_u16(buf: &[u8], offset: usize, section: &'static str) -> Result<u16> {
    require_len(buf, offset, 2, section)?;
    Ok(u16_at(buf, offset))
}

#[inline]
pub fn read_u32(buf: &[u8], offset: usize, section: &'static str) -> Result<u32> {
    require_len(buf, offset, 4, section)?;
    Ok(u32_at(buf, offset))
}

/// Reads at an offset a bounds check already covered; panics past the end.
#[inline]
pub fn u16_at(buf: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([buf[offset], buf[offset + 1]])
}

/// Reads at an offset a bounds check already covered; panics past the end.
#[inline]
pub fn u32_at(buf: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
    ])
}

/// Reads at an offset a bounds check already covered; panics past the end.
#[inline]
pub fn i32_at(buf: &[u8], offset: usize) -> i32 {
    u32_at(buf, offset) as i32
}

#[inline]
pub fn write_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

#[inline]
pub fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
