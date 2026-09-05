// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::Write;
use std::ops::Range;

use reseam_dex::file::DexBytes;

use super::{MAX_TYPE_ENTRIES, RES_TABLE_TYPE_SPEC};
use crate::buf::{read_u32_le, require_len, write_u16, write_u32};
use crate::chunk::write_header;
use crate::error::{invalid, Result};

const HEADER_LEN: usize = 16;

/// A `ResTable_typeSpec` chunk: the per-entry flags in place plus flags for
/// entries added after parse.
#[derive(Debug, Clone)]
pub struct TypeSpec {
    pub id: u8,
    data: DexBytes,
    flags_start: usize,
    raw_len: usize,
    extra: Vec<u32>,
}

impl TypeSpec {
    pub fn new(id: u8, flags: Vec<u32>) -> Self {
        Self {
            id,
            data: DexBytes::default(),
            flags_start: 0,
            raw_len: 0,
            extra: flags,
        }
    }

    pub(super) fn parse(data: &DexBytes, chunk: Range<usize>, header_size: usize) -> Result<Self> {
        let buf = &data.as_bytes()[chunk.clone()];
        require_len(buf, 0, header_size.max(HEADER_LEN), "type spec")?;
        let entry_count = read_u32_le(buf, 12, "type spec")? as usize;
        if entry_count > MAX_TYPE_ENTRIES {
            return Err(invalid("type spec", "entry count exceeds safety limit"));
        }
        require_len(buf, header_size, entry_count * 4, "type spec flags")?;
        Ok(Self {
            id: buf[8],
            data: data.clone(),
            flags_start: chunk.start + header_size,
            raw_len: entry_count,
            extra: Vec::new(),
        })
    }

    pub fn len(&self) -> usize {
        self.raw_len + self.extra.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn push(&mut self, flag: u32) {
        self.extra.push(flag);
    }

    pub(super) fn size(&self) -> usize {
        HEADER_LEN + self.len() * 4
    }

    pub(super) fn write(&self, out: &mut dyn Write) -> Result<()> {
        let mut head = Vec::with_capacity(HEADER_LEN);
        write_header(
            &mut head,
            RES_TABLE_TYPE_SPEC,
            HEADER_LEN as u16,
            self.size(),
        );
        head.push(self.id);
        head.push(0);
        write_u16(&mut head, 0);
        write_u32(&mut head, self.len() as u32);
        out.write_all(&head)?;
        if self.raw_len > 0 {
            out.write_all(
                &self.data.as_bytes()[self.flags_start..self.flags_start + self.raw_len * 4],
            )?;
        }
        let mut extra = Vec::with_capacity(self.extra.len() * 4);
        for flag in &self.extra {
            write_u32(&mut extra, *flag);
        }
        out.write_all(&extra)?;
        Ok(())
    }
}
