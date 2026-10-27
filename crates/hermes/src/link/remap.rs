// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::literals::Literals;
use crate::error::{HermesError, Result, invalid};
use crate::opcode::{IdKind, Instruction, Op};
use reseam_bytes::u32_at;

/// IDs of one kind in a linked module: `count` of them, moved to start at `base`.
#[derive(Clone, Copy)]
pub(super) struct Moved {
    pub base: u32,
    pub count: u32,
}

impl Moved {
    fn id(self, value: u64, at: usize) -> Result<u32> {
        if value >= u64::from(self.count) {
            return Err(invalid(at, "ID out of range"));
        }
        self.base
            .checked_add(value as u32)
            .ok_or(HermesError::Limit("four billion IDs of one kind"))
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
    /// Rewrites every ID operand of `inst`, in a body that starts at `start` in the module,
    /// copying the literals it reads.
    pub(super) fn instruction(&mut self, inst: &mut Instruction, start: usize) -> Result<()> {
        let at = start + inst.offset.expect("linked instructions are decoded") as usize;
        let read_values = literal_count(inst, self.shape_table, at)?;
        for (operand, value) in inst.definition().operands.iter().zip(&mut inst.values) {
            *value = u64::from(match operand.id {
                IdKind::None => continue,
                IdKind::String => *self
                    .strings
                    .get(*value as usize)
                    .ok_or_else(|| invalid(at, "string ID out of range"))?,
                IdKind::Function => self.functions.id(*value, at)?,
                IdKind::BigInt => self.bigints.id(*value, at)?,
                IdKind::RegExp => self.regexps.id(*value, at)?,
                IdKind::Shape => self.shapes.id(*value, at)?,
                IdKind::Switch => self.switches.id(*value, at)?,
                IdKind::ValueBuffer => self.values.relocate(
                    *value as u32,
                    read_values.ok_or_else(|| invalid(at, "value buffer without a length"))?,
                )?,
            });
        }
        Ok(())
    }
}

/// How many literals an instruction reads from the value buffer: an array's
/// static elements, or one value per property of an object's shape.
/// `at` is where `inst` is in the module.
pub(super) fn literal_count(
    inst: &Instruction,
    shapes: &[[u8; 8]],
    at: usize,
) -> Result<Option<u32>> {
    let shape = |index: u64| {
        shapes
            .get(index as usize)
            .map(|entry| u32_at(entry, 4))
            .ok_or_else(|| invalid(at, "shape ID out of range"))
    };
    Ok(match inst.op {
        Op::NewArrayWithBuffer | Op::NewArrayWithBufferLong => Some(inst.values[2] as u32),
        Op::NewObjectWithBuffer | Op::NewObjectWithBufferLong => Some(shape(inst.values[1])?),
        Op::NewObjectWithBufferAndParent => Some(shape(inst.values[2])?),
        _ => None,
    })
}
