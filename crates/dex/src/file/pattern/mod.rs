// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

mod opcode_matcher;

use std::ops::Range;

pub use opcode_matcher::OpcodeMatcher;

#[derive(Debug, Clone)]
pub enum InstructionPattern {
    Any,
    Opcode(OpcodeMatcher),
    OpcodeValue(u16),
}

impl InstructionPattern {
    pub fn matches(&self, opcode: Option<u16>) -> bool {
        match self {
            Self::Any => true,
            Self::Opcode(matcher) => matcher.opcode() == opcode,
            Self::OpcodeValue(value) => opcode == Some(*value),
        }
    }
}

/// The span of the first window of `opcodes` matching `pattern`.
pub(super) fn find_pattern_span(
    opcodes: &[Option<u16>],
    pattern: &[InstructionPattern],
) -> Option<Range<usize>> {
    if pattern.is_empty() {
        return Some(0..0);
    }

    if opcodes.len() < pattern.len() {
        return None;
    }

    opcodes
        .windows(pattern.len())
        .position(|window| {
            window
                .iter()
                .zip(pattern)
                .all(|(opcode, pattern)| pattern.matches(*opcode))
        })
        .map(|start| start..start + pattern.len())
}

#[cfg(test)]
mod tests {

    use super::{find_pattern_span, InstructionPattern, OpcodeMatcher};
    use crate::types::instruction::Instruction;

    fn opcodes(instructions: &[Instruction]) -> Vec<Option<u16>> {
        instructions.iter().map(Instruction::opcode).collect()
    }

    #[test]
    fn finds_pattern_span() {
        let instructions = opcodes(&[
            Instruction::Nop,
            Instruction::Const4 { dest: 0, value: 1 },
            Instruction::Return { src: 0 },
        ]);
        let pattern = [
            InstructionPattern::Opcode(OpcodeMatcher::Const4),
            InstructionPattern::Opcode(OpcodeMatcher::Return),
        ];

        assert_eq!(find_pattern_span(&instructions, &pattern), Some(1..3));
    }

    #[test]
    fn matches_raw_instruction_by_variant() {
        let instructions = opcodes(&[Instruction::RawInstruction {
            code_units: Box::new([0x1234, 0x5678]),
        }]);
        let pattern = [InstructionPattern::Opcode(OpcodeMatcher::RawInstruction)];

        assert_eq!(find_pattern_span(&instructions, &pattern), Some(0..1));
    }
}
