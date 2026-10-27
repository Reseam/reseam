// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::error::{Result, invalid};

pub(crate) fn slice(bytes: &[u8], offset: usize, length: usize) -> Result<&[u8]> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| invalid(offset, "range overflow"))?;
    bytes
        .get(offset..end)
        .ok_or_else(|| invalid(offset, "truncated section"))
}

pub(crate) fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let data = slice(bytes, offset, 4)?;
    Ok(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
}

pub(crate) const fn align(offset: usize) -> usize {
    (offset + 3) & !3
}
