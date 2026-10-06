// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{EntryOffsets, FLAG_OFFSET16, FLAG_SPARSE, HEADER_LEN, ResType};
use crate::buf::{read_u16_le, read_u32_le, require_len};
use crate::error::{Result, invalid, malformed};
use reseam_storage::Bytes;
use std::collections::BTreeMap;
use std::ops::Range;
impl ResType {
    pub(in crate::resources) fn parse(
        data: &Bytes,
        chunk: Range<usize>,
        header_size: usize,
    ) -> Result<Self> {
        let buf = &data.as_bytes()[chunk.clone()];
        require_len(buf, 0, header_size.max(HEADER_LEN), "res type")?;
        if header_size < HEADER_LEN {
            return Err(malformed("res type", chunk.start, "header is too short"));
        }
        let entry_count = read_u32_le(buf, 12, "res type")? as usize;
        let entries_start = read_u32_le(buf, 16, "res type")? as usize;
        if entries_start > buf.len() {
            return Err(malformed(
                "res type",
                16,
                "entries start is outside type chunk",
            ));
        }
        let flags = buf[9];
        let index_width = if flags & FLAG_SPARSE == 0 && flags & FLAG_OFFSET16 != 0 {
            2
        } else {
            4
        };
        let index_len = entry_count
            .checked_mul(index_width)
            .ok_or_else(|| invalid("res type", "offset index size overflows"))?;
        require_len(buf, header_size, index_len, "res type offsets")?;
        if entries_start < header_size + index_len {
            return Err(malformed(
                "res type",
                chunk.start + 16,
                "entry data overlaps the offset index",
            ));
        }
        let config_end = header_size.min(buf.len());
        if config_end > HEADER_LEN && config_end - HEADER_LEN < 4 {
            return Err(malformed(
                "res type",
                HEADER_LEN,
                "config data is shorter than the size field",
            ));
        }
        let offsets = if flags & FLAG_SPARSE != 0 {
            EntryOffsets::Sparse { count: entry_count }
        } else if flags & FLAG_OFFSET16 != 0 {
            EntryOffsets::Dense16
        } else {
            EntryOffsets::Dense32
        };
        let raw_len = match &offsets {
            EntryOffsets::Sparse { count } => {
                if *count == 0 {
                    0
                } else {
                    usize::from(read_u16_le(
                        buf,
                        header_size + (count - 1) * 4,
                        "res sparse index",
                    )?) + 1
                }
            }
            EntryOffsets::Dense32 | EntryOffsets::Dense16 => entry_count,
        };
        Ok(Self {
            id: buf[8],
            data: data.clone(),
            chunk: chunk.clone(),
            header_size,
            config: None,
            raw_len,
            entries_start: chunk.start + entries_start,
            offsets,
            overlay: BTreeMap::new(),
            len: raw_len,
        })
    }
}
