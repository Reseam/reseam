// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;

use crate::bytes::read_u32;
use crate::error::{Result, invalid};
use crate::literal::{Literal, MAX_RUN, read_literals};
use crate::opcode::{Instruction, Op};

pub(super) fn at(inst: &Instruction) -> usize {
    inst.offset.expect("decoded instructions have offsets") as usize
}

pub(super) fn checked_id(value: u64, count: u32, base: u32) -> Result<u32> {
    if value >= u64::from(count) {
        return Err(invalid(0, "linked ID out of range"));
    }
    base.checked_add(value as u32)
        .ok_or_else(|| invalid(0, "linked ID overflow"))
}

/// How many literals an instruction reads from the value buffer: an array's
/// static elements, or one value per property of an object's shape.
pub(super) fn literal_count(inst: &Instruction, shapes: &[[u8; 8]]) -> Result<Option<u32>> {
    let shape = |index: u64| {
        shapes
            .get(index as usize)
            .map(|entry| read_u32(entry, 4))
            .ok_or_else(|| invalid(at(inst), "shape ID out of range"))?
    };
    Ok(match inst.op {
        Op::NewArrayWithBuffer | Op::NewArrayWithBufferLong => Some(inst.values[2] as u32),
        Op::NewObjectWithBuffer | Op::NewObjectWithBufferLong => Some(shape(inst.values[1])?),
        Op::NewObjectWithBufferAndParent => Some(shape(inst.values[2])?),
        _ => None,
    })
}

/// A linked module's literal buffer, copied per reference. Hermes shares
/// byte-identical literal sequences, so a reference may start inside another
/// run; each one is read from its own offset and length, as the runtime reads
/// it, and copied with its string IDs remapped. Short strings become long
/// strings, since remapped IDs need not fit 16 bits.
pub(super) struct Literals<'a> {
    source: &'a [u8],
    strings: &'a [u32],
    base: u32,
    pub(super) output: Vec<u8>,
    copies: BTreeMap<(u32, u32), u32>,
}

impl<'a> Literals<'a> {
    pub(super) fn new(source: &'a [u8], strings: &'a [u32], base: u32) -> Self {
        Self {
            source,
            strings,
            base,
            output: Vec::new(),
            copies: BTreeMap::new(),
        }
    }

    /// The offset of the copy of `count` literals read from `offset`.
    pub(super) fn relocate(&mut self, offset: u32, count: u32) -> Result<u32> {
        if let Some(&copy) = self.copies.get(&(offset, count)) {
            return Ok(copy);
        }
        let copy = u32::try_from(self.output.len())
            .ok()
            .and_then(|length| self.base.checked_add(length))
            .ok_or_else(|| invalid(offset as usize, "literal buffer offset overflow"))?;
        let literals = read_literals(self.source, offset as usize, count as usize)?;
        for run in literals.chunk_by(|a, b| a.kind == b.kind) {
            for run in run.chunks(MAX_RUN) {
                let kind = match run[0].kind {
                    Literal::ShortString => Literal::LongString,
                    kind => kind,
                };
                if run.len() > usize::from(Literal::LENGTH) {
                    self.output
                        .push(Literal::LONG_RUN | kind.tag() | (run.len() >> 8) as u8);
                    self.output.push(run.len() as u8);
                } else {
                    self.output.push(kind.tag() | run.len() as u8);
                }
                for literal in run {
                    match literal.kind {
                        Literal::ShortString | Literal::LongString => {
                            let id = self.strings.get(literal.string_id()? as usize).ok_or_else(
                                || invalid(offset as usize, "literal string ID out of range"),
                            )?;
                            self.output.extend_from_slice(&id.to_le_bytes());
                        }
                        _ => self.output.extend_from_slice(literal.bytes),
                    }
                }
            }
        }
        self.copies.insert((offset, count), copy);
        Ok(copy)
    }
}
