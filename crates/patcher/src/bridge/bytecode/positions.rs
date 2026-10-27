// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use boltffi::export;
use reseam_dex::{DexFile, Instruction as DexInsn};

use super::lookup::{opcode_patterns, query_opcode};
use crate::bridge::handles::with_code;
use crate::bridge::run_state::checked;

fn position(m: u32, start: u32, matches: impl Fn(&DexFile, &DexInsn) -> bool) -> Option<u32> {
    with_code(m, |dex, code| {
        code.instructions()
            .iter()
            .enumerate()
            .skip(start as usize)
            .find(|(_, insn)| matches(dex, insn))
            .map(|(i, _)| i as u32)
    })
}

fn position_reversed(m: u32, before: u32, matches: impl Fn(&DexInsn) -> bool) -> Option<u32> {
    with_code(m, |_, code| {
        let end = (before as usize).min(code.instructions().len());
        code.instructions()[..end]
            .iter()
            .rposition(&matches)
            .map(|i| i as u32)
    })
}

#[export]
pub fn index_of_first(m: u32, start: u32, op: u16) -> Option<u32> {
    position(m, start, |_, insn| insn.opcode() == Some(op))
}

#[export]
pub fn index_of_first_reversed(m: u32, start: u32, op: u16) -> Option<u32> {
    position_reversed(m, start, |insn| insn.opcode() == Some(op))
}

#[export]
pub fn index_of_first_literal(m: u32, literal: i64) -> Option<u32> {
    position(m, 0, |_, insn| insn.literal() == Some(literal))
}

#[export]
pub fn index_of_first_literal_reversed(m: u32, literal: i64) -> Option<u32> {
    position_reversed(m, u32::MAX, |insn| insn.literal() == Some(literal))
}

#[export]
pub fn index_of_first_string(m: u32, s: String) -> Option<u32> {
    position(m, 0, |dex, insn| {
        insn.string_ref()
            .is_some_and(|idx| dex.strings().get(idx) == s)
    })
}

#[export]
pub fn index_of_first_string_with_prefix(m: u32, prefix: String) -> Option<u32> {
    position(m, 0, |dex, insn| {
        insn.string_ref()
            .is_some_and(|idx| dex.strings().get(idx).starts_with(&prefix))
    })
}

#[export]
pub fn find_all_indices(m: u32, op: u16) -> Vec<u32> {
    with_code(m, |_, code| {
        Some(
            code.instructions()
                .iter()
                .enumerate()
                .filter(|(_, insn)| insn.opcode() == Some(op))
                .map(|(i, _)| i as u32)
                .collect(),
        )
    })
    .unwrap_or_default()
}

#[export]
pub fn index_of_first_method_call(
    m: u32,
    defining_class: String,
    method_name: String,
    start: u32,
) -> Option<u32> {
    position(m, start, |dex, insn| {
        insn.method_ref().is_some_and(|idx| {
            let method = dex.method_id(idx);
            dex.type_descriptor(method.class) == defining_class
                && dex.strings().get(method.name) == method_name
        })
    })
}

#[export]
pub fn index_of_first_field_access(
    m: u32,
    op: i32,
    field_type: Option<String>,
    defining_class: Option<String>,
    start: u32,
) -> Option<u32> {
    let op = checked(query_opcode(op).map(Some))?;
    position(m, start, |dex, insn| {
        op.is_none_or(|op| insn.opcode() == Some(op))
            && insn.field_ref().is_some_and(|idx| {
                let field = dex.field_id(idx);
                field_type
                    .as_deref()
                    .is_none_or(|t| dex.type_descriptor(field.type_) == t)
                    && defining_class
                        .as_deref()
                        .is_none_or(|c| dex.type_descriptor(field.class) == c)
            })
    })
}

#[export]
pub fn index_of_opcode_sequence(m: u32, opcodes: Vec<i32>, start: u32) -> Option<u32> {
    let pattern = checked(opcode_patterns(&opcodes).map(Some))?;
    with_code(m, |_, code| {
        if pattern.is_empty() {
            return None;
        }
        code.instructions()
            .windows(pattern.len())
            .enumerate()
            .skip(start as usize)
            .find(|(_, window)| {
                window
                    .iter()
                    .zip(&pattern)
                    .all(|(insn, expected)| expected.matches(insn.opcode()))
            })
            .map(|(i, _)| i as u32)
    })
}
