// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::{Result, slice};

pub(crate) fn read_u8(buf: &[u8], off: usize, section: &'static str) -> Result<u8> {
    Ok(slice(buf, off, 1, section)?[0])
}

pub(crate) fn read_u16(buf: &[u8], off: usize) -> Result<u16> {
    Ok(u16_at(slice(buf, off, 2, "dex data")?, 0))
}

pub(crate) fn read_u32(buf: &[u8], off: usize) -> Result<u32> {
    Ok(u32_at(slice(buf, off, 4, "dex data")?, 0))
}

pub(crate) fn u16_at(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}

pub(crate) fn u32_at(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

pub(crate) fn i32_at(buf: &[u8], off: usize) -> i32 {
    u32_at(buf, off) as i32
}
