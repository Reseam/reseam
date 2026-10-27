// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::opcodes::{FILL_ARRAY_DATA_PAYLOAD, PACKED_SWITCH_PAYLOAD, SPARSE_SWITCH_PAYLOAD};
use reseam_bytes::{require_len, u16_at, u32_at};

use super::query::opcode_units;
use crate::error::{Result, malformed};
use crate::model::{FieldIdx, MethodIdx, Pool, StringIdx, TypeIdx};

/// One instruction located in a code item's instruction stream.
#[derive(Debug, Clone, Copy)]
pub struct RawInstruction {
    pub index: usize,
    pub opcode: u8,
    unit_off: usize,
    unit0: u16,
    pub(crate) units: usize,
}

impl RawInstruction {
    /// Byte offset of the instruction's first code unit.
    pub fn offset(&self) -> usize {
        self.unit_off
    }

    /// The opcode, with switch and fill-array payloads reported by their
    /// pseudo-opcode (`0x0100`, `0x0200`, `0x0300`) like
    /// [`crate::instruction::Instruction::opcode`] does.
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
        Some(operand.read(buf, self.unit_off))
    }

    /// The method, field or string the instruction refers to, if any.
    pub(crate) fn member_or_string_ref(&self, buf: &[u8]) -> Option<(Pool, u32)> {
        let operand = index_operands(self.opcode)
            .iter()
            .find(|operand| matches!(operand.pool, Pool::Method | Pool::Field | Pool::String))?;
        Some((operand.pool, operand.read(buf, self.unit_off)))
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

impl IndexOperand {
    fn read(&self, buf: &[u8], unit_off: usize) -> u32 {
        match self.width {
            IndexWidth::U16 => u32::from(u16_at(buf, unit_off + self.at)),
            IndexWidth::U32 => u32_at(buf, unit_off + self.at),
        }
    }
}

macro_rules! define_index_operands {
    ($($variant:ident [$($shape:tt)*] [$($definition:tt)*] => $opname:ident $opcode:expr, $units:tt; [$($register:ident: $reg_type:ident $kind:ident $access:ident ($max:expr)),*]; $args:ident; [$($index:ident: $id_type:ident $pool:ident $at:literal $index_width:ident),*]; $format:ident($($slot:ident),*);)*) => {
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
super::catalogue::instruction_catalogue!(define_index_operands);

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

pub(crate) struct InstructionStream<'a> {
    buf: &'a [u8],
    offset: usize,
    index: usize,
}

pub(crate) fn instruction_stream(
    buf: &[u8],
    start: usize,
    size: usize,
) -> Result<InstructionStream<'_>> {
    let bytes = size
        .checked_mul(2)
        .ok_or_else(|| malformed("code item instruction", start, "stream size overflow"))?;
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
                malformed("code item instruction", offset, "instruction size overflow")
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

/// Counts the instructions in a code item by walking opcode lengths, without
/// decoding operands or building instructions.
pub fn count_instructions(buf: &[u8], start: usize, insns_size: usize) -> Result<u32> {
    instruction_stream(buf, start, insns_size)?.try_fold(0, |count, frame| frame.map(|_| count + 1))
}

fn payload_units(buf: &[u8], unit_off: usize, unit0: u16) -> Result<usize> {
    Ok(match unit0 {
        PACKED_SWITCH_PAYLOAD => {
            require_len(buf, unit_off, 4, "packed-switch payload")?;
            4 + u16_at(buf, unit_off + 2) as usize * 2
        }
        SPARSE_SWITCH_PAYLOAD => {
            require_len(buf, unit_off, 4, "sparse-switch payload")?;
            2 + u16_at(buf, unit_off + 2) as usize * 4
        }
        FILL_ARRAY_DATA_PAYLOAD => {
            require_len(buf, unit_off, 8, "fill-array-data payload")?;
            let data_bytes = (u32_at(buf, unit_off + 4) as usize)
                .checked_mul(u16_at(buf, unit_off + 2) as usize)
                .and_then(|bytes| bytes.checked_add(8))
                .ok_or_else(|| {
                    malformed("fill-array-data payload", unit_off, "payload size overflow")
                })?;
            data_bytes.div_ceil(2)
        }
        _ => 1,
    })
}
