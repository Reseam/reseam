// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Opcode metadata generated from the corresponding Hermes release definition.

use crate::error::{Result, invalid};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperandKind {
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
    pub(crate) const fn width(self) -> usize {
        match self {
            Self::Reg8 | Self::UInt8 | Self::Addr8 => 1,
            Self::UInt16 => 2,
            Self::Double => 8,
            Self::Reg32 | Self::UInt32 | Self::Addr32 | Self::Imm32 => 4,
        }
    }

    pub(crate) const fn is_register(self) -> bool {
        matches!(self, Self::Reg8 | Self::Reg32)
    }

    pub(crate) const fn is_address(self) -> bool {
        matches!(self, Self::Addr8 | Self::Addr32)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IdKind {
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
pub(crate) struct Operand {
    pub kind: OperandKind,
    pub id: IdKind,
}

#[derive(Debug)]
pub(crate) struct Opcode {
    pub op: Op,
    pub operands: &'static [Operand],
    /// Operand positions of the registers the instruction writes.
    pub writes: &'static [usize],
    /// Encodings of the same instruction with wider operands, narrowest first.
    pub wider: &'static [Op],
}

impl Opcode {
    pub(crate) fn size(&self) -> usize {
        1 + self.operands.iter().map(|o| o.kind.width()).sum::<usize>()
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

    fn opcodes(self) -> &'static [Opcode] {
        match self {
            Self::V98 => &V98,
        }
    }

    fn opcode(self, op: Op) -> &'static Opcode {
        match self {
            Self::V98 => &V98[op as usize],
        }
    }

    fn code(self, op: Op) -> u8 {
        match self {
            Self::V98 => op as u8,
        }
    }
}

/// A decoded instruction. Integers retain their raw little-endian bits;
/// signed addresses use two's complement and doubles use IEEE 754 bits.
#[derive(Debug, Clone)]
pub(crate) struct Instruction {
    pub version: BytecodeVersion,
    /// Byte offset in the original body; `None` for generated instructions.
    pub offset: Option<u32>,
    pub op: Op,
    pub values: Vec<u64>,
}

impl Instruction {
    pub(crate) fn new(op: Op, values: &[u64]) -> Self {
        Self {
            version: BytecodeVersion::V98,
            offset: None,
            op,
            values: values.to_vec(),
        }
    }

    pub(crate) fn definition(&self) -> &'static Opcode {
        self.version.opcode(self.op)
    }

    pub(crate) fn code(&self) -> u8 {
        self.version.code(self.op)
    }

    pub(crate) fn operands(&self) -> impl Iterator<Item = (Operand, u64)> {
        self.definition()
            .operands
            .iter()
            .copied()
            .zip(self.values.iter().copied())
    }

    pub(crate) fn written_registers(&self) -> impl Iterator<Item = u32> {
        self.definition()
            .writes
            .iter()
            .map(|&operand| self.values[operand] as u32)
    }

    /// This instruction's encodings, narrowest first.
    pub(crate) fn encodings(&self) -> impl Iterator<Item = &'static Opcode> {
        let definition = self.definition();
        std::iter::once(definition)
            .chain(definition.wider.iter().map(|&op| self.version.opcode(op)))
    }
}

/// Decodes a function's instruction stream, stopping before appended switch tables.
/// Unknown opcodes and truncated operands report their byte offsets.
pub(crate) fn decode(version: BytecodeVersion, bytes: &[u8]) -> Result<Vec<Instruction>> {
    let mut offset = 0;
    let mut instructions = Vec::new();
    while offset < bytes.len() {
        let definition = version
            .opcodes()
            .get(usize::from(bytes[offset]))
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
            offset: Some(offset as u32),
            op: definition.op,
            values,
        });
        offset = end;
    }
    Ok(instructions)
}
