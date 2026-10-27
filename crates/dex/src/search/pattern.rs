// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::ops::Range;

use crate::error::Result;
use crate::file::DexFile;
use crate::instruction::catalogue::instruction_catalogue;
use crate::search::index::RefQuery;
use crate::search::view::MethodHit;

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

fn find_pattern_span(
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

impl DexFile {
    pub fn find_methods_with_opcodes(
        &self,
        opcodes: &[InstructionPattern],
    ) -> Result<Vec<MethodHit>> {
        self.scan_methods_collect(&RefQuery::default(), |view| {
            if !view.has_code() {
                return Ok(None);
            }
            let mut sequence = Vec::new();
            view.opcodes(&mut sequence)?;
            Ok(find_pattern_span(&sequence, opcodes).map(|_| view.hit()))
        })
    }
}

macro_rules! define_opcode_matchers {
    ($($variant:ident [$($shape:tt)*] [$($definition:tt)*] => $opname:ident $opcode:expr, $units:tt; [$($register:ident: $reg_type:ident $kind:ident $access:ident ($max:expr)),*]; $args:ident; [$($index:ident: $id_type:ident $pool:ident $at:literal $index_width:ident),*]; $format:ident($($slot:ident),*);)*) => {
        #[derive(Debug, Clone, Copy)]
        pub enum OpcodeMatcher { $($variant,)* }
        impl OpcodeMatcher {
            const fn opcode(self) -> Option<u16> {
                match self { $(Self::$variant => $opcode,)* }
            }
        }
    };
}
instruction_catalogue!(define_opcode_matchers);
