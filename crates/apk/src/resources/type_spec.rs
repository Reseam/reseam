// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod reader;
mod writer;

use std::ops::Range;

use reseam_storage::Bytes;

const HEADER_LEN: usize = 16;

/// A `ResTable_typeSpec` chunk: the per-entry flags in place plus flags for
/// entries added after parse.
#[derive(Debug, Clone)]
pub struct TypeSpec {
    pub(super) id: u8,
    data: Bytes,
    flags_start: usize,
    raw_len: usize,
    extra: Vec<u32>,
    chunk: Range<usize>,
}

impl TypeSpec {
    pub fn new(id: u8, flags: Vec<u32>) -> Self {
        Self {
            id,
            data: Bytes::default(),
            flags_start: 0,
            raw_len: 0,
            extra: flags,
            chunk: 0..0,
        }
    }

    pub fn id(&self) -> u8 {
        self.id
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
}
