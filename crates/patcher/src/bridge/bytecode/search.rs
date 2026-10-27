// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::bridge::run_state::checked;
use boltffi::export;
use reseam_dex::Instruction as DexInsn;

use crate::bridge::dex::convert::{self, dex_to_kotlin};
use crate::bridge::dex::types::{
    FieldPattern, Instruction, InstructionHit, MethodPattern, MethodRef,
};
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
    with_code(m, |dex, code| {
        let mut seen = rustc_hash::FxHashSet::default();
        Some(
            code.instructions()
                .iter()
                .filter_map(DexInsn::method_ref)
                .filter(|idx| seen.insert(*idx))
                .map(|idx| convert::resolve_method_ref(dex, idx))
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
pub fn find_calls_matching(pattern: MethodPattern) -> Vec<InstructionHit> {
    with_method_query(&pattern, |query| {
        hits(with_ctx(|ctx| checked(ctx.find_calls_matching(query))))
    })
}

#[export]
pub fn find_calls_in(m: u32, pattern: MethodPattern) -> Vec<u32> {
    let Some(location) = method_location(m) else {
        return Vec::new();
    };
    with_method_query(&pattern, |query| {
        indices(with_ctx(|ctx| checked(ctx.find_calls_in(location, query))))
    })
}

#[export]
pub fn find_field_accesses_matching(pattern: FieldPattern) -> Vec<InstructionHit> {
    hits(with_ctx(|ctx| {
        checked(ctx.find_field_accesses_matching(&field_query(&pattern)))
    }))
}

#[export]
pub fn find_field_accesses_in(m: u32, pattern: FieldPattern) -> Vec<u32> {
    let Some(location) = method_location(m) else {
        return Vec::new();
    };
    indices(with_ctx(|ctx| {
        checked(ctx.find_field_accesses_in(location, &field_query(&pattern)))
    }))
}

fn with_method_query<R>(pattern: &MethodPattern, f: impl FnOnce(&MethodRefQuery<'_>) -> R) -> R {
    let parameters = pattern.parameters.as_deref().map(strs);
    let required_parameters = strs(&pattern.required_parameters);
    let owner_assignable_to = strs(&pattern.owner_assignable_to);
    f(&MethodRefQuery {
        owner: pattern.owner.as_deref(),
        name: pattern.name.as_deref(),
        return_type: pattern.return_type.as_deref(),
        parameters: parameters.as_deref(),
        required_parameters: &required_parameters,
        parameter_count: pattern.parameter_count.map(|count| count as usize),
        owner_assignable_to: &owner_assignable_to,
    })
}

fn strs(values: &[String]) -> Vec<&str> {
    values.iter().map(String::as_str).collect()
}

fn field_query(pattern: &FieldPattern) -> FieldRefQuery<'_> {
    FieldRefQuery {
        owner: pattern.owner.as_deref(),
        name: pattern.name.as_deref(),
        field_type: pattern.field_type.as_deref(),
    }
}

fn indices(indices: Vec<usize>) -> Vec<u32> {
    indices.into_iter().map(|index| index as u32).collect()
}
