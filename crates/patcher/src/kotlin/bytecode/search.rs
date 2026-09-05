// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reading instructions of one method, and locating instructions across
//! the whole app.

use boltffi::export;
use reseam_apk::reseam_dex::{DexFile, Instruction as DexInsn};

use super::lookup::opcode_patterns;
use crate::context::{InstructionLocation, SiteHit};
use crate::kotlin::convert::dex_to_kotlin;
use crate::kotlin::handles::{alloc_method, alloc_methods, with_code, with_ctx};
use crate::kotlin::types::{
    FieldRef, Instruction, InstructionHit, MethodCallSiteResult, MethodRef, SimpleInsn,
};

#[export]
pub fn get_instructions(m: u32) -> Vec<Instruction> {
    with_code(m, |dex, code| {
        Some(
            code.instructions
                .iter()
                .map(|insn| dex_to_kotlin(insn, dex))
                .collect(),
        )
    })
    .unwrap_or_default()
}

/// The instruction at `index`, or a `nop` when there is none.
#[export]
pub fn get_instruction(m: u32, index: u32) -> Instruction {
    with_code(m, |dex, code| {
        code.instructions
            .get(index as usize)
            .map(|insn| dex_to_kotlin(insn, dex))
    })
    .unwrap_or(Instruction::Simple(SimpleInsn { opcode: 0 }))
}

#[export]
pub fn instruction_count(m: u32) -> u32 {
    with_code(m, |_, code| Some(code.instructions.len() as u32)).unwrap_or(0)
}

fn position(m: u32, start: u32, matches: impl Fn(&DexFile, &DexInsn) -> bool) -> Option<u32> {
    with_code(m, |dex, code| {
        code.instructions
            .iter()
            .enumerate()
            .skip(start as usize)
            .find(|(_, insn)| matches(dex, insn))
            .map(|(i, _)| i as u32)
    })
}

fn position_reversed(m: u32, before: u32, matches: impl Fn(&DexInsn) -> bool) -> Option<u32> {
    with_code(m, |_, code| {
        let end = (before as usize).min(code.instructions.len());
        code.instructions[..end]
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
        insn.string_ref().is_some_and(|idx| dex.string(idx) == s)
    })
}

#[export]
pub fn find_all_indices(m: u32, op: u16) -> Vec<u32> {
    with_code(m, |_, code| {
        Some(
            code.instructions
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
                && dex.string(method.name) == method_name
        })
    })
}

/// A field access matching every given filter; `op` below zero matches any opcode.
#[export]
pub fn index_of_first_field_access(
    m: u32,
    op: i32,
    field_type: Option<String>,
    defining_class: Option<String>,
    start: u32,
) -> Option<u32> {
    let op = u16::try_from(op).ok();
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

/// The first index at or after `start` where `opcodes` match consecutively;
/// negative opcodes match anything.
#[export]
pub fn index_of_opcode_sequence(m: u32, opcodes: Vec<i32>, start: u32) -> Option<u32> {
    let pattern = opcode_patterns(&opcodes);
    with_code(m, |_, code| {
        if pattern.is_empty() {
            return None;
        }
        code.instructions
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

#[export]
pub fn find_instructions_by_literal(literal: i64) -> Vec<InstructionHit> {
    hits(with_ctx(|ctx| ctx.find_instructions_by_literal(literal)))
}

#[export]
pub fn find_instructions_by_string(s: String) -> Vec<InstructionHit> {
    hits(with_ctx(|ctx| ctx.find_instructions_by_string(&s)))
}

#[export]
pub fn find_instructions_by_string_contains(substring: String) -> Vec<InstructionHit> {
    hits(with_ctx(|ctx| {
        ctx.find_instructions_by_string_contains(&substring)
    }))
}

#[export]
pub fn find_instructions_by_resource_id(res_type: String, res_name: String) -> Vec<InstructionHit> {
    match with_ctx(|ctx| {
        ctx.apk_mut()
            .find_resource(&res_type, &res_name)
            .ok()
            .flatten()
    }) {
        Some((_, res_id)) => find_instructions_by_literal(res_id as i64),
        None => Vec::new(),
    }
}

/// Call sites of `(class_names[i], method_names[i])` pairs.
#[export]
pub fn find_method_call_sites(
    class_names: Vec<String>,
    method_names: Vec<String>,
) -> Vec<MethodCallSiteResult> {
    let targets: Vec<(String, String)> = class_names.into_iter().zip(method_names).collect();
    site_results(with_ctx(|ctx| ctx.find_method_call_sites(&targets)))
}

/// Accesses of `(class_names[i], field_names[i])` pairs.
#[export]
pub fn find_field_access_sites(
    class_names: Vec<String>,
    field_names: Vec<String>,
) -> Vec<MethodCallSiteResult> {
    let targets: Vec<(String, String)> = class_names.into_iter().zip(field_names).collect();
    site_results(with_ctx(|ctx| ctx.find_field_access_sites(&targets)))
}

#[export]
pub fn find_instructions_by_invoke(
    defining_class: String,
    method_name: String,
) -> Vec<InstructionHit> {
    find_method_call_sites(vec![defining_class], vec![method_name])
        .into_iter()
        .map(|site| InstructionHit {
            method: site.method,
            index: site.index,
        })
        .collect()
}

#[export]
pub fn all_method_handles() -> Vec<u32> {
    let mut locations = Vec::new();
    with_ctx(|ctx| ctx.for_each_method(|location| locations.push(location)));
    alloc_methods(locations)
}

#[export]
pub fn instruction_string_ref(m: u32, index: u32) -> Option<String> {
    with_code(m, |dex, code| {
        let idx = code.instructions.get(index as usize)?.string_ref()?;
        Some(dex.string(idx).into_owned())
    })
}

#[export]
pub fn instruction_method_ref(m: u32, index: u32) -> Option<MethodRef> {
    with_code(m, |dex, code| {
        let method = dex.method_id(code.instructions.get(index as usize)?.method_ref()?);
        Some(MethodRef {
            defining_class: dex.type_descriptor(method.class).into_owned(),
            name: dex.string(method.name).into_owned(),
            proto: dex.proto_descriptor(&dex.proto(method.proto)),
        })
    })
}

#[export]
pub fn instruction_field_ref(m: u32, index: u32) -> Option<FieldRef> {
    with_code(m, |dex, code| {
        let field = dex.field_id(code.instructions.get(index as usize)?.field_ref()?);
        Some(FieldRef {
            defining_class: dex.type_descriptor(field.class).into_owned(),
            name: dex.string(field.name).into_owned(),
            field_type: dex.type_descriptor(field.type_).into_owned(),
        })
    })
}

#[export]
pub fn instruction_type_ref(m: u32, index: u32) -> Option<String> {
    with_code(m, |dex, code| {
        let idx = code.instructions.get(index as usize)?.type_ref()?;
        Some(dex.type_descriptor(idx).into_owned())
    })
}

fn hits(locations: Vec<InstructionLocation>) -> Vec<InstructionHit> {
    locations
        .into_iter()
        .map(|loc| InstructionHit {
            method: alloc_method(loc.method),
            index: loc.insn_idx as u32,
        })
        .collect()
}

fn site_results(sites: Vec<SiteHit>) -> Vec<MethodCallSiteResult> {
    sites
        .into_iter()
        .map(|hit| MethodCallSiteResult {
            method: alloc_method(hit.loc.method),
            index: hit.loc.insn_idx as u32,
            target_index: hit.target_index as u32,
        })
        .collect()
}
