// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{HEADER_LEN, TypeSpec};
use crate::buf::{read_u32_le, require_len};
use crate::error::{Result, invalid};
use reseam_storage::Bytes;
use std::ops::Range;

impl TypeSpec {
    pub(in crate::resources) fn parse(
        data: &Bytes,
        chunk: Range<usize>,
        header_size: usize,
    ) -> Result<Self> {
        let buf = &data.as_bytes()[chunk.clone()];
        require_len(buf, 0, header_size.max(HEADER_LEN), "type spec")?;
        if header_size < HEADER_LEN {
            return Err(invalid("type spec", "header is shorter than 16 bytes"));
        }
        let entry_count = read_u32_le(buf, 12, "type spec")? as usize;
        require_len(
            buf,
            header_size,
            entry_count
                .checked_mul(4)
                .ok_or_else(|| invalid("type spec", "flag index size overflows"))?,
            "type spec flags",
        )?;
        Ok(Self {
            id: buf[8],
            data: data.clone(),
            flags_start: chunk.start + header_size,
            raw_len: entry_count,
            extra: Vec::new(),
            chunk,
        })
    }
}
