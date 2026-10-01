// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::decode::{opcode_units, payload_units};
use crate::error::{Result, require_len};
use crate::read::{u16_at, u32_at};
use crate::types::{FieldIdx, MethodIdx, Pool, StringIdx, TypeIdx};

/// One instruction located in a code item's instruction stream.
#[derive(Debug, Clone, Copy)]
pub struct RawInstruction {
    pub index: usize,
    pub opcode: u8,
    unit_off: usize,
    unit0: u16,
    pub(super) units: usize,
}

impl RawInstruction {
    /// Byte offset of the instruction's first code unit.
    pub fn offset(&self) -> usize {
        self.unit_off
    }

    /// The opcode, with switch and fill-array payloads reported by their
    /// pseudo-opcode (`0x0100`, `0x0200`, `0x0300`) like
    /// [`crate::types::instruction::Instruction::opcode`] does.
    pub fn opcode(&self) -> Option<u16> {
        if self.opcode == 0 {
            return matches!(self.unit0, 0 | 0x0100 | 0x0200 | 0x0300).then_some(self.unit0);
        }
        opcode_units(self.opcode).map(|_| u16::from(self.opcode))
    }

    pub fn method_ref(&self, buf: &[u8]) -> Option<MethodIdx> {
        self.index_ref(buf, Pool::Method).map(MethodIdx)
    }
    pub fn field_ref(&self, buf: &[u8]) -> Option<FieldIdx> {
        self.index_ref(buf, Pool::Field).map(FieldIdx)
    }
    pub fn string_ref(&self, buf: &[u8]) -> Option<StringIdx> {
        self.index_ref(buf, Pool::String).map(StringIdx)
    }
    pub fn type_ref(&self, buf: &[u8]) -> Option<TypeIdx> {
        self.index_ref(buf, Pool::Type).map(TypeIdx)
    }

    fn index_ref(&self, buf: &[u8], pool: Pool) -> Option<u32> {
        let operand = index_operands(self.opcode)
            .iter()
            .find(|operand| operand.pool == pool)?;
        Some(match operand.width {
            IndexWidth::U16 => u32::from(u16_at(buf, self.unit_off + operand.at)),
            IndexWidth::U32 => u32_at(buf, self.unit_off + operand.at),
        })
    }

    /// The literal of a `const*` or `*-int/lit*` instruction, with the same
    /// value [`crate::types::instruction::Instruction::literal`] reports for
    /// its decoded form.
    pub fn literal(&self, buf: &[u8]) -> Option<i64> {
        let at = |units: usize| self.unit_off + units * 2;
        Some(match self.opcode {
            0x12 => i64::from(((u16_at(buf, at(0)) >> 12) as u8 as i8) << 4 >> 4),
            0x13 | 0x16 | 0xd0..=0xd7 => i64::from(u16_at(buf, at(1)) as i16),
            0x15 => i64::from(u16_at(buf, at(1)) as i16) << 16,
            0x19 => i64::from(u16_at(buf, at(1)) as i16) << 48,
            0x14 | 0x17 => i64::from(u32_at(buf, at(1)) as i32),
            0x18 => i64::from(u32_at(buf, at(1))) | i64::from(u32_at(buf, at(3))) << 32,
            0xd8..=0xe2 => i64::from((u16_at(buf, at(1)) >> 8) as i8),
            _ => return None,
        })
    }
}
#[derive(Debug, Clone, Copy)]
pub(crate) enum IndexWidth {
    U16,
    U32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct IndexOperand {
    pub pool: Pool,
    pub at: usize,
    pub width: IndexWidth,
}

macro_rules! define_index_operands {
    ($($variant:ident [$($shape:tt)*] [$($definition:tt)*] => $opname:ident $opcode:expr, $units:tt; [$($register:ident: $reg_type:ident $kind:ident $access:ident ($max:expr)),*]; $args:ident; [$($index:ident: $id_type:ident $pool:ident $at:literal $index_width:ident),*];)*) => {
        pub(crate) fn index_operands(opcode: u8) -> &'static [IndexOperand] {
            const OPERANDS: [&[IndexOperand]; 256] = {
                let mut operands = [&[] as &[IndexOperand]; 256];
                $(let opcode: Option<u16> = $opcode;
                if let Some(opcode) = opcode {
                    if opcode < 256 {
                        operands[opcode as usize] = &[$(IndexOperand { pool: Pool::$pool, at: $at, width: IndexWidth::$index_width },)*];
                    }
                })*
                operands
            };
            OPERANDS[usize::from(opcode)]
        }
    };
}
crate::types::instruction_catalogue::instruction_catalogue!(define_index_operands);

/// Visits every instruction of a code item's stream, stopping early when
/// `visit` returns `false`.
pub fn walk_instructions(
    buf: &[u8],
    start: usize,
    insns_size: usize,
    mut visit: impl FnMut(&RawInstruction) -> bool,
) -> Result<()> {
    for instruction in instruction_stream(buf, start, insns_size)? {
        if !visit(&instruction?) {
            break;
        }
    }
    Ok(())
}

pub(super) struct InstructionStream<'a> {
    buf: &'a [u8],
    offset: usize,
    index: usize,
}

pub(super) fn instruction_stream(
    buf: &[u8],
    start: usize,
    size: usize,
) -> Result<InstructionStream<'_>> {
    let bytes = size.checked_mul(2).ok_or_else(|| {
        crate::error::malformed("code item instruction", start, "stream size overflow")
    })?;
    require_len(buf, start, bytes, "code item instruction")?;
    Ok(InstructionStream {
        buf: &buf[..start + bytes],
        offset: start,
        index: 0,
    })
}

impl Iterator for InstructionStream<'_> {
    type Item = Result<RawInstruction>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset == self.buf.len() {
            return None;
        }
        let offset = self.offset;
        let framed = (|| {
            require_len(self.buf, offset, 2, "code item instruction")?;
            let unit0 = u16_at(self.buf, offset);
            let opcode = unit0 as u8;
            let units = if opcode == 0 {
                payload_units(self.buf, offset, unit0)?
            } else {
                opcode_units(opcode).unwrap_or(1)
            };
            let bytes = units.checked_mul(2).ok_or_else(|| {
                crate::error::malformed(
                    "code item instruction",
                    offset,
                    "instruction size overflow",
                )
            })?;
            require_len(self.buf, offset, bytes, "code item instruction")?;
            Ok(RawInstruction {
                index: self.index,
                opcode,
                unit_off: offset,
                unit0,
                units,
            })
        })();
        self.offset = framed
            .as_ref()
            .map_or(self.buf.len(), |frame| offset + frame.units * 2);
        self.index += 1;
        Some(framed)
    }
}
