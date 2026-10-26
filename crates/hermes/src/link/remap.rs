// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::literals::Literals;
use crate::bytes::read_u32;
use crate::error::{Result, invalid};
use crate::opcode::{IdKind, Instruction, Op};

/// IDs of one kind in a linked module: `count` of them, moved to start at `base`.
#[derive(Clone, Copy)]
pub(super) struct Moved {
    pub base: u32,
    pub count: u32,
}

impl Moved {
    fn id(self, value: u64) -> Result<u32> {
        if value >= u64::from(self.count) {
            return Err(invalid(0, "linked ID out of range"));
        }
        self.base
            .checked_add(value as u32)
            .ok_or_else(|| invalid(0, "linked ID overflow"))
    }
}

/// Where each reference of a linked module moves in the merged file.
pub(super) struct Remap<'m> {
    /// The merged ID of each module string.
    pub strings: &'m [u32],
    pub functions: Moved,
    pub bigints: Moved,
    pub regexps: Moved,
    pub shapes: Moved,
    pub switches: Moved,
    pub values: Literals<'m>,
    /// The module's object shapes, which say how many values an object literal reads.
    pub shape_table: &'m [[u8; 8]],
}

impl Remap<'_> {
    /// Rewrites every ID operand of `inst`, copying the literals it reads.
    pub(super) fn instruction(&mut self, inst: &mut Instruction) -> Result<()> {
        let at = at(inst);
        let read_values = literal_count(inst, self.shape_table)?;
        for (operand, value) in inst.definition().operands.iter().zip(&mut inst.values) {
            *value = u64::from(match operand.id {
                IdKind::None => continue,
                IdKind::String => *self
                    .strings
                    .get(*value as usize)
                    .ok_or_else(|| invalid(at, "string ID out of range"))?,
                IdKind::Function => self.functions.id(*value)?,
                IdKind::BigInt => self.bigints.id(*value)?,
                IdKind::RegExp => self.regexps.id(*value)?,
                IdKind::Shape => self.shapes.id(*value)?,
                IdKind::Switch => self.switches.id(*value)?,
                IdKind::ValueBuffer => self.values.relocate(
                    *value as u32,
                    read_values.ok_or_else(|| invalid(at, "value buffer without a length"))?,
                )?,
            });
        }
        Ok(())
    }
}

fn at(inst: &Instruction) -> usize {
    inst.offset.expect("decoded instructions have offsets") as usize
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
