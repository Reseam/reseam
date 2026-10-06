// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

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
