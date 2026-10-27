// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::bytes::align;
use crate::error::{Result, invalid};
use crate::function::Function;
use crate::opcode::{Instruction, Op};
use crate::string::StringId;
use reseam_bytes::slice;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SwitchKind {
    Integer,
    String,
}

impl SwitchKind {
    const fn stride(self) -> usize {
        match self {
            Self::Integer => 4,
            Self::String => 8,
        }
    }
}

/// The jump table a switch instruction appends after its function's code.
pub(crate) struct SwitchTable<'a> {
    pub kind: SwitchKind,
    /// The operand holding the table's offset relative to the instruction.
    pub operand: usize,
    pub offset: usize,
    pub bytes: &'a [u8],
}

impl SwitchTable<'_> {
    /// Each case's string key, for string switches, and relative jump target.
    pub(crate) fn entries(&self) -> impl Iterator<Item = (Option<StringId>, i32)> {
        self.bytes.chunks_exact(self.kind.stride()).map(|entry| {
            let word = |at: usize| {
                u32::from_le_bytes(
                    entry[at..at + 4]
                        .try_into()
                        .expect("entries hold whole words"),
                )
            };
            match self.kind {
                SwitchKind::Integer => (None, word(0).cast_signed()),
                SwitchKind::String => (Some(StringId(word(0))), word(4).cast_signed()),
            }
        })
    }
}

pub(crate) fn switch_table<'a>(
    function: &Function<'a>,
    inst: &Instruction,
) -> Result<Option<SwitchTable<'a>>> {
    let (kind, operand, count) = match inst.op {
        Op::UIntSwitchImm => (
            SwitchKind::Integer,
            1,
            inst.values[4]
                .checked_sub(inst.values[3])
                .and_then(|n| n.checked_add(1)),
        ),
        Op::StringSwitchImm => (SwitchKind::String, 2, Some(inst.values[4])),
        _ => return Ok(None),
    };
    let at = function.header.offset as usize
        + inst
            .offset
            .expect("switch tables belong to decoded instructions") as usize;
    let count = count.ok_or_else(|| invalid(at, "invalid switch range"))?;
    let offset = usize::try_from(inst.values[operand])
        .ok()
        .and_then(|relative| at.checked_add(relative))
        .filter(|&n| n <= usize::MAX - 3)
        .map(align)
        .ok_or_else(|| invalid(at, "switch offset overflow"))?;
    let size = usize::try_from(count)
        .ok()
        .and_then(|n| n.checked_mul(kind.stride()))
        .ok_or_else(|| invalid(offset, "switch table overflow"))?;
    Ok(Some(SwitchTable {
        kind,
        operand,
        offset,
        bytes: slice(function.source, offset, size, "switch table")?,
    }))
}
