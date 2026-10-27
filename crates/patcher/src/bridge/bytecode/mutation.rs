// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;

use boltffi::export;
use reseam_dex::{AccessFlags, CodeItem, Instruction as DexInsn};

use crate::bridge::dex::convert::kotlin_to_dex;
use crate::bridge::dex::link::link_instructions;
use crate::bridge::dex::types::{Instruction, MethodEdit};
use crate::bridge::handles::with_method_mut;
use crate::context::{code_mut, method_mut};

use super::logged;

fn edit_code<R>(
    m: u32,
    insns: &[Instruction],
    f: impl FnOnce(&mut CodeItem, Vec<DexInsn>) -> Result<R, String>,
) -> Result<R, String> {
    link_instructions(insns);
    let result = with_method_mut(m, |dex, loc| {
        let insns = logged(
            "convert instructions",
            insns
                .iter()
                .map(|insn| kotlin_to_dex(insn, dex, Some(loc.dex_idx)))
                .collect(),
        )?;
        let code = logged("access method", code_mut(dex, loc))?
            .ok_or_else(|| format!("method handle {m} has no code"))?;
        f(code, insns).map(Some)
    });
    crate::bridge::run_state::check_call()?;
    result.ok_or_else(|| format!("method handle {m} could not be edited"))
}

#[export]
pub fn replace_body(
    m: u32,
    registers_size: u16,
    outs_size: u16,
    insns: Vec<Instruction>,
) -> Result<(), String> {
    link_instructions(&insns);
    let outcome = with_method_mut(m, |dex, loc| {
        let method = logged("access method", method_mut(dex, loc))?
            .ok_or_else(|| format!("method handle {m} is missing"))?;
        let (id, flags) = (method.method, method.access_flags);
        let proto = dex.proto_descriptor(&dex.proto(dex.method_id(id).proto));
        let incoming = logged(
            "replacement frame",
            super::registers::incoming_words(&proto, flags),
        )?;
        let code = logged(
            "replace body",
            super::assembly::assemble(insns, dex, Some(loc.dex_idx), |code, _| {
                code.set_register_frame(registers_size, incoming, outs_size)
            }),
        )?;
        let method = logged("replace method", method_mut(dex, loc))?
            .expect("assembly leaves the method slot in place");
        method.code = Some(Arc::new(code));
        method
            .access_flags
            .remove(AccessFlags::NATIVE | AccessFlags::ABSTRACT);
        Ok(Some(()))
    });
    crate::bridge::run_state::check_call()?;
    outcome.ok_or_else(|| format!("method handle {m} could not be replaced"))
}

/// How an edit that left the frame as it was moved the method's instructions.
/// How growing the frame by `registers` locals at `base` moved the method's instructions.
pub(super) fn growth_mapping(
    base: u16,
    registers: u16,
    mapping: reseam_dex::InstructionMap,
) -> Result<MethodEdit, String> {
    Ok(MethodEdit {
        register_base: base,
        register_shift: registers,
        ..edit_mapping(mapping)?
    })
}

pub(super) fn edit_mapping(mapping: reseam_dex::InstructionMap) -> Result<MethodEdit, String> {
    fn indices(values: &[usize]) -> Result<Vec<u32>, String> {
        values
            .iter()
            .map(|index| {
                u32::try_from(*index).map_err(|_| "instruction index exceeds 32 bits".into())
            })
            .collect()
    }
    Ok(MethodEdit {
        register_base: 0,
        register_shift: 0,
        starts: indices(mapping.starts())?,
        instructions: indices(mapping.instructions())?,
        ends: indices(mapping.ends())?,
    })
}

#[export]
pub fn insert_instructions(
    m: u32,
    index: u32,
    insns: Vec<Instruction>,
) -> Result<MethodEdit, String> {
    edit_code(m, &insns, |code, insns| {
        edit_mapping(logged(
            "insert instructions",
            code.insert_instructions(index as usize, &insns),
        )?)
    })
}

#[export]
pub fn replace_instruction(
    m: u32,
    index: u32,
    insns: Vec<Instruction>,
) -> Result<MethodEdit, String> {
    edit_code(m, &insns, |code, insns| {
        edit_mapping(logged(
            "replace instruction",
            code.replace_instructions(index as usize, &insns),
        )?)
    })
}

#[export]
pub fn remove_instructions(m: u32, index: u32, count: u32) -> Result<MethodEdit, String> {
    let result = with_method_mut(m, |dex, loc| {
        let code = logged("access method", code_mut(dex, loc))?
            .ok_or_else(|| format!("method handle {m} has no code"))?;
        edit_mapping(logged(
            "remove instructions",
            code.remove_instructions(index as usize, count as usize),
        )?)
        .map(Some)
    });
    crate::bridge::run_state::check_call()?;
    result.ok_or_else(|| format!("method handle {m} could not be edited"))
}
