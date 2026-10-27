// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::bridge::run_state::checked;
use boltffi::export;
use reseam_dex::{DexFile, Instruction as DexInsn};

use crate::bridge::dex::convert::{self, dex_to_kotlin};
use crate::bridge::dex::types::{FieldRef, Instruction, InstructionHit, MethodRef};
use crate::bridge::handles::{
    alloc_method, alloc_methods, method_location, with_code, with_instruction,
};
use crate::bridge::phase::with_ctx;
use crate::context::{FieldRefQuery, InstructionLocation, MethodRefQuery};

#[export]
pub fn get_instructions(m: u32) -> Vec<Instruction> {
    with_code(m, |dex, code| {
        let dex_index = method_location(m)?.dex_idx;
        Some(checked(
            code.instructions()
                .iter()
                .map(|insn| dex_to_kotlin(insn, dex, Some(dex_index)))
                .collect::<reseam_dex::Result<Vec<_>>>(),
        ))
    })
    .unwrap_or_default()
}

#[export]
pub fn method_references(m: u32) -> Vec<MethodRef> {
    distinct_references(m, DexInsn::method_ref, convert::resolve_method_ref)
}

#[export]
pub fn field_references(m: u32) -> Vec<FieldRef> {
    distinct_references(m, DexInsn::field_ref, convert::resolve_field_ref)
}

fn distinct_references<I: Copy + Eq + std::hash::Hash, R>(
    m: u32,
    reference: impl Fn(&DexInsn) -> Option<I>,
    resolve: impl Fn(&DexFile, I) -> R,
) -> Vec<R> {
    with_code(m, |dex, code| {
        let mut seen = rustc_hash::FxHashSet::default();
        Some(
            code.instructions()
                .iter()
                .filter_map(&reference)
                .filter(|idx| seen.insert(*idx))
                .map(|idx| resolve(dex, idx))
                .collect(),
        )
    })
    .unwrap_or_default()
}

#[export]
pub fn find_methods_by_string_prefix(prefix: String) -> Vec<u32> {
    alloc_methods(with_ctx(|ctx| {
        checked(ctx.find_methods_by_string_prefix(&prefix))
    }))
}

#[export]
pub fn count_following_check_cast(
    sites: Vec<InstructionHit>,
    type_descriptor: String,
    look_ahead: u32,
) -> u32 {
    sites
        .iter()
        .filter(|site| {
            with_code(site.method, |dex, code| {
                Some(
                    code.instructions()
                        .iter()
                        .skip((site.index as usize).saturating_add(1))
                        .take(look_ahead as usize)
                        .any(|instruction| {
                            matches!(instruction, DexInsn::CheckCast { type_, .. }
                                if dex.type_descriptor(*type_) == type_descriptor)
                        }),
                )
            })
            .unwrap_or(false)
        })
        .count() as u32
}

#[export]
pub fn find_instructions_by_literal(literal: i64) -> Vec<InstructionHit> {
    hits(with_ctx(|ctx| {
        checked(ctx.find_instructions_by_literal(literal))
    }))
}

#[export]
pub fn find_instructions_by_string(s: String) -> Vec<InstructionHit> {
    hits(with_ctx(|ctx| checked(ctx.find_instructions_by_string(&s))))
}

#[export]
pub fn find_instructions_by_string_contains(substring: String) -> Vec<InstructionHit> {
    hits(with_ctx(|ctx| {
        checked(ctx.find_instructions_by_string_contains(&substring))
    }))
}

#[export]
pub fn find_instructions_by_string_prefix(prefix: String) -> Vec<InstructionHit> {
    hits(with_ctx(|ctx| {
        checked(ctx.find_instructions_by_string_prefix(&prefix))
    }))
}

#[export]
pub fn find_field_accesses_matching(
    owner: Option<String>,
    name: Option<String>,
    field_type: Option<String>,
) -> Vec<InstructionHit> {
    hits(with_ctx(|ctx| {
        checked(ctx.find_field_accesses_matching(&FieldRefQuery {
            owner: owner.as_deref(),
            name: name.as_deref(),
            field_type: field_type.as_deref(),
        }))
    }))
}

#[export]
pub fn all_method_handles() -> Vec<u32> {
    let mut locations = Vec::new();
    with_ctx(|ctx| checked(ctx.for_each_method(|location| locations.push(location))));
    alloc_methods(locations)
}

#[export]
pub fn instruction_string_ref(m: u32, index: u32) -> Option<String> {
    with_instruction(m, index, |dex, instruction| {
        let idx = instruction.string_ref()?;
        Some(dex.strings().get(idx).into_owned())
    })
}

#[export]
pub fn instruction_method_ref(m: u32, index: u32) -> Option<MethodRef> {
    with_instruction(m, index, |dex, instruction| {
        let method = dex.method_id(instruction.method_ref()?);
        Some(MethodRef {
            defining_class: dex.type_descriptor(method.class).into_owned(),
            name: dex.strings().get(method.name).into_owned(),
            proto: dex.proto_descriptor(&dex.proto(method.proto)),
        })
    })
}

#[export]
pub fn instruction_field_ref(m: u32, index: u32) -> Option<FieldRef> {
    with_instruction(m, index, |dex, instruction| {
        let field = dex.field_id(instruction.field_ref()?);
        Some(FieldRef {
            defining_class: dex.type_descriptor(field.class).into_owned(),
            name: dex.strings().get(field.name).into_owned(),
            field_type: dex.type_descriptor(field.type_).into_owned(),
        })
    })
}

#[export]
pub fn instruction_type_ref(m: u32, index: u32) -> Option<String> {
    with_instruction(m, index, |dex, instruction| {
        let idx = instruction.type_ref()?;
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

#[export]
pub fn find_calls_matching(
    owner: Option<String>,
    name: Option<String>,
    return_type: Option<String>,
    parameters: Option<Vec<String>>,
    required_parameters: Vec<String>,
    parameter_count: Option<u32>,
) -> Vec<InstructionHit> {
    let parameters: Option<Vec<&str>> = parameters
        .as_ref()
        .map(|v| v.iter().map(String::as_str).collect());
    let required_parameters: Vec<&str> = required_parameters.iter().map(String::as_str).collect();
    hits(with_ctx(|ctx| {
        checked(ctx.find_calls_matching(&MethodRefQuery {
            owner: owner.as_deref(),
            name: name.as_deref(),
            return_type: return_type.as_deref(),
            parameters: parameters.as_deref(),
            required_parameters: &required_parameters,
            parameter_count: parameter_count.map(|count| count as usize),
        }))
    }))
}
