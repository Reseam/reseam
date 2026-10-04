//! Opcode metadata generated from the corresponding Hermes release definition.

use crate::error::{Result, invalid};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperandKind {
    Reg8,
    Reg32,
    UInt8,
    UInt16,
    UInt32,
    Addr8,
    Addr32,
    Imm32,
    Double,
}

impl OperandKind {
    pub const fn width(self) -> usize {
        match self {
            Self::Reg8 | Self::UInt8 | Self::Addr8 => 1,
            Self::UInt16 => 2,
            Self::Double => 8,
            _ => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdKind {
    None,
    String,
    Function,
    BigInt,
    RegExp,
    Shape,
    ValueBuffer,
    Switch,
}

#[derive(Debug, Clone, Copy)]
pub struct Operand {
    pub kind: OperandKind,
    pub id: IdKind,
}

#[derive(Debug)]
pub struct Opcode {
    pub name: &'static str,
    pub operands: &'static [Operand],
}

impl Opcode {
    pub fn size(&self) -> usize {
        1 + self.operands.iter().map(|o| o.kind.width()).sum::<usize>()
    }
}

include!(concat!(env!("OUT_DIR"), "/opcodes.rs"));

/// A decoded instruction. Integers retain their raw little-endian bits;
/// signed addresses use two's complement and doubles use IEEE 754 bits.
#[derive(Debug, Clone)]
pub struct Instruction {
    pub offset: u32,
    pub opcode: u8,
    pub values: Vec<u64>,
}

impl Instruction {
    pub fn definition(&self) -> &'static Opcode {
        &V98[usize::from(self.opcode)]
    }
}

/// Decodes a function's instruction stream, stopping before appended switch tables.
/// Unknown opcodes and truncated operands report their byte offsets.
pub fn decode(bytes: &[u8]) -> Result<Vec<Instruction>> {
    let mut offset = 0;
    let mut instructions = Vec::new();
    while offset < bytes.len() {
        let code = bytes[offset];
        let definition = V98
            .get(usize::from(code))
            .ok_or_else(|| invalid(offset, "unknown opcode"))?;
        let end = offset + definition.size();
        let data = bytes
            .get(offset + 1..end)
            .ok_or_else(|| invalid(offset, "truncated instruction"))?;
        let mut cursor = 0;
        let values = definition
            .operands
            .iter()
            .map(|operand| {
                let width = operand.kind.width();
                let mut value = [0; 8];
                value[..width].copy_from_slice(&data[cursor..cursor + width]);
                cursor += width;
                u64::from_le_bytes(value)
            })
            .collect();
        instructions.push(Instruction {
            offset: offset as u32,
            opcode: code,
            values,
        });
        offset = end;
    }
    Ok(instructions)
}
