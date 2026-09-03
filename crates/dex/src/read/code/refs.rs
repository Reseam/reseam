// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Instruction walking without decoding: opcode, length and the one operand
//! searches ask about (a pool index or a literal) straight from code units.

use super::decode::{opcode_units, payload_units};
use super::format::{u16_at, u32_at};
use crate::error::{require_len, Result};
use crate::types::{FieldIdx, MethodIdx, StringIdx, TypeIdx};

/// One instruction located in a code item's instruction stream.
#[derive(Debug, Clone, Copy)]
pub struct RawInstruction {
    pub index: usize,
    pub opcode: u8,
    unit_off: usize,
    unit0: u16,
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
        Some(if self.opcode == 0x00 && matches!(self.unit0, 0x0100 | 0x0200 | 0x0300) {
            self.unit0
        } else {
            self.opcode as u16
        })
    }

    pub fn method_ref(&self, buf: &[u8]) -> Option<MethodIdx> {
        matches!(self.opcode, 0x6e..=0x72 | 0x74..=0x78 | 0xfa | 0xfb)
            .then(|| MethodIdx(self.index_operand(buf)))
    }

    pub fn field_ref(&self, buf: &[u8]) -> Option<FieldIdx> {
        matches!(self.opcode, 0x52..=0x6d).then(|| FieldIdx(self.index_operand(buf)))
    }

    pub fn string_ref(&self, buf: &[u8]) -> Option<StringIdx> {
        match self.opcode {
            0x1a => Some(StringIdx(self.index_operand(buf))),
            0x1b => Some(StringIdx(u32_at(buf, self.unit_off + 2))),
            _ => None,
        }
    }

    pub fn type_ref(&self, buf: &[u8]) -> Option<TypeIdx> {
        matches!(self.opcode, 0x1c | 0x1f | 0x20 | 0x22..=0x25)
            .then(|| TypeIdx(self.index_operand(buf)))
    }

    /// The literal of a `const*` or `*-int/lit*` instruction, with the same
    /// value [`crate::types::instruction::Instruction::literal`] reports for
    /// its decoded form.
    pub fn literal(&self, buf: &[u8]) -> Option<i64> {
        let at = |units: usize| self.unit_off + units * 2;
        Some(match self.opcode {
            0x12 => i64::from(((u16_at(buf, at(0)) >> 12) as u8 as i8) << 4 >> 4),
            0x13 | 0x15 | 0x16 | 0x19 | 0xd0..=0xd7 => i64::from(u16_at(buf, at(1)) as i16),
            0x14 | 0x17 => i64::from(u32_at(buf, at(1)) as i32),
            0x18 => i64::from(u32_at(buf, at(1))) | i64::from(u32_at(buf, at(3))) << 32,
            0xd8..=0xe2 => i64::from((u16_at(buf, at(1)) >> 8) as i8),
            _ => return None,
        })
    }

    fn index_operand(&self, buf: &[u8]) -> u32 {
        u16_at(buf, self.unit_off + 2) as u32
    }
}

/// Visits every instruction of a code item's stream, stopping early when
/// `visit` returns `false`.
pub fn walk_instructions(
    buf: &[u8],
    start: usize,
    insns_size: usize,
    mut visit: impl FnMut(&RawInstruction) -> bool,
) -> Result<()> {
    let mut pc = 0usize;
    let mut index = 0usize;
    while pc < insns_size {
        let unit_off = start + pc * 2;
        require_len(buf, unit_off, 2, "code item instruction")?;
        let unit0 = u16_at(buf, unit_off);
        let opcode = (unit0 & 0xFF) as u8;
        let units = if opcode == 0x00 {
            payload_units(buf, unit_off, unit0)?
        } else {
            opcode_units(opcode)
        };
        require_len(buf, unit_off, units * 2, "code item instruction")?;
        if !visit(&RawInstruction {
            index,
            opcode,
            unit_off,
            unit0,
        }) {
            return Ok(());
        }
        pc += units;
        index += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read::code::decode::decode_instructions;
    use crate::write::instruction_writer::encode_instructions;
    use crate::types::instruction::{Instruction, RegList};

    fn units(instructions: &[Instruction]) -> Vec<u8> {
        encode_instructions(instructions)
            .unwrap()
            .iter()
            .flat_map(|u| u.to_le_bytes())
            .collect()
    }

    #[test]
    fn raw_operands_match_decoded_instructions() {
        let program = vec![
            Instruction::Const4 { dest: 0, value: -3 },
            Instruction::Const16 { dest: 1, value: -300 },
            Instruction::Const { dest: 2, value: 0x1234_5678 },
            Instruction::ConstHigh16 { dest: 3, value: -2 },
            Instruction::ConstWide16 { dest: 4, value: 7 },
            Instruction::ConstWide32 { dest: 6, value: -70_000 },
            Instruction::ConstWide { dest: 8, value: -0x1122_3344_5566_7788 },
            Instruction::ConstWideHigh16 { dest: 10, value: 0x4000 },
            Instruction::ConstString { dest: 0, string: StringIdx(5) },
            Instruction::ConstStringJumbo { dest: 0, string: StringIdx(0x1_0002) },
            Instruction::ConstClass { dest: 0, type_: TypeIdx(9) },
            Instruction::AddIntLit8 { dest: 0, src: 1, literal: -5 },
            Instruction::MulIntLit16 { dest: 0, src: 1, literal: 1000 },
            Instruction::Sget { dest: 0, field: FieldIdx(77) },
            Instruction::InvokeStatic { method: MethodIdx(4242), args: RegList::new() },
            Instruction::InvokeVirtualRange { method: MethodIdx(11), first_reg: 0, count: 2 },
            Instruction::ReturnVoid,
        ];
        let buf = units(&program);
        let decoded = decode_instructions(&buf, 0, buf.len() / 2).unwrap();
        let mut seen = 0;
        walk_instructions(&buf, 0, buf.len() / 2, |raw| {
            let insn = &decoded[raw.index];
            assert_eq!(raw.opcode(), insn.opcode());
            assert_eq!(raw.literal(&buf), insn.literal());
            assert_eq!(raw.string_ref(&buf), insn.string_ref());
            assert_eq!(raw.type_ref(&buf), insn.type_ref());
            assert_eq!(raw.field_ref(&buf), insn.field_ref());
            assert_eq!(raw.method_ref(&buf), insn.method_ref());
            seen += 1;
            true
        })
        .unwrap();
        assert_eq!(seen, program.len());
    }
}
