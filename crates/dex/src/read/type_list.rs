// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::error::Result;
use crate::model::{TypeIdx, TypeList};
use reseam_bytes::{read_u32, u16_at, u32_at};

pub(crate) fn read_type_list(buf: &[u8], off: usize) -> TypeList {
    let size = u32_at(buf, off) as usize;
    (0..size)
        .map(|i| TypeIdx(u32::from(u16_at(buf, off + 4 + i * 2))))
        .collect()
}

pub(crate) fn validate_type_list(buf: &[u8], off: usize) -> Result<()> {
    let size = read_u32(buf, off, "type list")? as usize;
    crate::error::require_array(buf, off + 4, size, 2, "type list")?;
    Ok(())
}
