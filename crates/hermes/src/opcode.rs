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

    pub(crate) fn writes_first_register(&self) -> bool {
        self.operands
            .first()
            .is_some_and(|o| matches!(o.kind, OperandKind::Reg8 | OperandKind::Reg32))
            && ![
                "Put",
                "Store",
                "Define",
                "Throw",
                "Ret",
                "IteratorClose",
                "UIntSwitch",
                "StringSwitch",
                "Reify",
            ]
            .iter()
            .any(|prefix| self.name.starts_with(prefix))
    }
}

include!(concat!(env!("OUT_DIR"), "/opcodes.rs"));

/// Execution bytecode formats understood by this crate. Each version selects
/// its own generated opcode and operand definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum BytecodeVersion {
    V98 = 98,
}

impl BytecodeVersion {
    pub(crate) fn parse(version: u32) -> Result<Self> {
        match version {
            98 => Ok(Self::V98),
            other => Err(crate::HermesError::Version(other)),
        }
    }

    pub(crate) const fn opcodes(self) -> &'static [Opcode] {
        match self {
            Self::V98 => V98,
        }
    }
}

/// A decoded instruction. Integers retain their raw little-endian bits;
/// signed addresses use two's complement and doubles use IEEE 754 bits.
#[derive(Debug, Clone)]
pub struct Instruction {
    pub(crate) version: BytecodeVersion,
    pub(crate) offset: u32,
    pub(crate) opcode: u8,
    pub(crate) values: Vec<u64>,
}

impl Instruction {
    pub fn offset(&self) -> u32 {
        self.offset
    }

    pub fn operands(&self) -> impl Iterator<Item = (Operand, u64)> {
        self.definition()
            .operands
            .iter()
            .copied()
            .zip(self.values.iter().copied())
    }
    pub fn definition(&self) -> &'static Opcode {
        &self.version.opcodes()[usize::from(self.opcode)]
    }
}

/// Decodes a function's instruction stream, stopping before appended switch tables.
/// Unknown opcodes and truncated operands report their byte offsets.
pub fn decode(version: BytecodeVersion, bytes: &[u8]) -> Result<Vec<Instruction>> {
    let mut offset = 0;
    let mut instructions = Vec::new();
    while offset < bytes.len() {
        let code = bytes[offset];
        let definition = version
            .opcodes()
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
            version,
            offset: offset as u32,
            opcode: code,
            values,
        });
        offset = end;
    }
    Ok(instructions)
}
