// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

mod opcode_matcher;

use std::ops::Range;

use crate::types::instruction::Instruction;

pub use opcode_matcher::OpcodeMatcher;

#[derive(Debug, Clone)]
pub enum InstructionPattern {
    Any,
    Opcode(OpcodeMatcher),
    OpcodeValue(u16),
}

impl InstructionPattern {
    fn matches(&self, instruction: &Instruction) -> bool {
        match self {
            Self::Any => true,
            Self::Opcode(matcher) => matcher.matches(instruction),
            Self::OpcodeValue(opcode) => instruction.opcode() == Some(*opcode),
        }
    }
}

pub(super) fn matches_pattern(
    instructions: &[Instruction],
    pattern: &[InstructionPattern],
) -> bool {
    find_pattern_span(instructions, pattern).is_some()
}

pub(super) fn find_pattern_span(
    instructions: &[Instruction],
    pattern: &[InstructionPattern],
) -> Option<Range<usize>> {
    if pattern.is_empty() {
        return Some(0..0);
    }

    if instructions.len() < pattern.len() {
        return None;
    }

    instructions
        .windows(pattern.len())
        .position(|window| {
            window
                .iter()
                .zip(pattern)
                .all(|(instruction, pattern)| pattern.matches(instruction))
        })
        .map(|start| start..start + pattern.len())
}

#[cfg(test)]
mod tests {

    use super::{find_pattern_span, matches_pattern, InstructionPattern, OpcodeMatcher};
    use crate::types::instruction::Instruction;

    #[test]
    fn finds_pattern_span() {
        let instructions = vec![
            Instruction::Nop,
            Instruction::Const4 { dest: 0, value: 1 },
            Instruction::Return { src: 0 },
        ];
        let pattern = [
            InstructionPattern::Opcode(OpcodeMatcher::Const4),
            InstructionPattern::Opcode(OpcodeMatcher::Return),
        ];

        assert!(matches_pattern(&instructions, &pattern));
        assert_eq!(find_pattern_span(&instructions, &pattern), Some(1..3));
    }

    #[test]
    fn matches_raw_instruction_by_variant() {
        let instructions = vec![Instruction::RawInstruction {
            code_units: Box::new([0x1234, 0x5678]),
        }];
        let pattern = [InstructionPattern::Opcode(OpcodeMatcher::RawInstruction)];

        assert!(matches_pattern(&instructions, &pattern));
        assert_eq!(find_pattern_span(&instructions, &pattern), Some(0..1));
    }
}
