// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::ops::Range;

use crate::FunctionId;
use crate::bytes::align;
use crate::error::{Result, invalid};
use crate::string::StringId;
use reseam_bytes::{read_u32, slice, u32_at};

mod flags;
mod header;

pub(crate) use flags::{FunctionFlags, FunctionKind, Prohibit};
pub(crate) use header::{FunctionHeader, LARGE_HEADER_SIZE, SMALL_HEADER_SIZE};

const EXCEPTION_HANDLER_SIZE: usize = 12;

/// An exception handler covering bytecode offsets `start..end`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ExceptionHandler {
    pub start: u32,
    pub end: u32,
    pub target: u32,
}

impl ExceptionHandler {
    pub(crate) fn covers(&self, offset: u32) -> bool {
        (self.start..self.end).contains(&offset)
    }

    /// The handler table that follows a large header: a count, then the handlers.
    pub(crate) fn encode_table(handlers: &[Self]) -> Vec<u8> {
        std::iter::once(handlers.len() as u32)
            .chain(handlers.iter().flat_map(|h| [h.start, h.end, h.target]))
            .flat_map(u32::to_le_bytes)
            .collect()
    }

    pub(crate) fn table_size(count: usize) -> usize {
        4 + count * EXCEPTION_HANDLER_SIZE
    }
}

/// A borrowed function view. Parameter count excludes the implicit `this`.
pub(crate) struct Function<'a> {
    pub id: FunctionId,
    pub header: FunctionHeader,
    pub source: &'a [u8],
}

impl Function<'_> {
    pub(crate) fn name(&self) -> StringId {
        StringId(self.header.name)
    }

    pub(crate) fn parameter_count(&self) -> u32 {
        self.header.parameters.saturating_sub(1)
    }

    pub(crate) fn body_range(&self) -> Range<usize> {
        let start = self.header.offset as usize;
        start..start + self.header.size as usize
    }

    pub(crate) fn instructions(&self) -> Result<Vec<crate::opcode::Instruction>> {
        crate::opcode::decode(&self.source[self.body_range()], self.header.offset as usize)
    }

    pub(crate) fn exception_handlers(&self) -> Result<Vec<ExceptionHandler>> {
        let Some(large) = self
            .header
            .large
            .filter(|_| self.header.flags.exception_handler)
        else {
            return Ok(Vec::new());
        };
        let position = align(large + LARGE_HEADER_SIZE);
        let count = read_u32(self.source, position, "exception table")? as usize;
        let size = count
            .checked_mul(EXCEPTION_HANDLER_SIZE)
            .ok_or_else(|| invalid(position, "exception table overflow"))?;
        slice(self.source, position + 4, size, "exception table")?
            .as_chunks::<EXCEPTION_HANDLER_SIZE>()
            .0
            .iter()
            .map(|entry| {
                Ok(ExceptionHandler {
                    start: u32_at(entry, 0),
                    end: u32_at(entry, 4),
                    target: u32_at(entry, 8),
                })
            })
            .collect()
    }
}
