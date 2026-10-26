// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;

use boltffi::export;
use reseam_apk::reseam_dex::types::code_rewrite::InstructionExpansion;
use reseam_apk::reseam_dex::{AccessFlags, CodeItem, Instruction as DexInsn};

use crate::kotlin::convert::kotlin_to_dex;
use crate::kotlin::handles::{code_mut, method_mut, with_method_mut};
use crate::kotlin::link::link_instructions;
use crate::kotlin::types::{Instruction, MethodEdit};

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
    crate::kotlin::handles::check_call()?;
    result.ok_or_else(|| format!("method handle {m} could not be edited"))
}

#[export]
pub fn set_instructions(m: u32, insns: Vec<Instruction>) -> Result<(), String> {
    edit_code(m, &insns, |code, insns| {
        code.set_instructions(insns);
        Ok(())
    })
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
            crate::kotlin::invoke::incoming_words(&proto, flags),
        )?;
        let code = logged(
            "replace body",
            super::assembly::assemble(insns, Vec::new(), dex, Some(loc.dex_idx), |code, _| {
                code.set_register_frame(registers_size, incoming, outs_size)
            }),
        )?
        .code;
        let method = logged("replace method", method_mut(dex, loc))?
            .expect("assembly leaves the method slot in place");
        method.code = Some(Arc::new(code));
        method
            .access_flags
            .remove(AccessFlags::NATIVE | AccessFlags::ABSTRACT);
        Ok(Some(()))
    });
    crate::kotlin::handles::check_call()?;
    outcome.ok_or_else(|| format!("method handle {m} could not be replaced"))
}

pub(super) fn edit_mapping(
    mapping: reseam_apk::reseam_dex::types::code_rewrite::InstructionMap,
) -> Result<MethodEdit, String> {
    fn indices(values: &[usize]) -> Result<Vec<u32>, String> {
        values
            .iter()
            .map(|index| {
                u32::try_from(*index).map_err(|_| "instruction index exceeds 32 bits".into())
            })
            .collect()
    }
    Ok(MethodEdit {
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
pub fn insert_before_instruction(
    m: u32,
    index: u32,
    insns: Vec<Instruction>,
) -> Result<MethodEdit, String> {
    edit_code(m, &insns, |code, insns| {
        let mut expansions: Vec<InstructionExpansion> = code
            .instructions()
            .iter()
            .cloned()
            .map(Into::into)
            .collect();
        let expansion = expansions
            .get_mut(index as usize)
            .ok_or_else(|| format!("instruction index {index} outside body"))?;
        expansion.before = insns;
        edit_mapping(logged(
            "insert before instruction",
            code.rewrite_instructions(expansions, &std::collections::BTreeMap::new()),
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
    crate::kotlin::handles::check_call()?;
    result.ok_or_else(|| format!("method handle {m} could not be edited"))
}

#[export]
pub fn return_early(m: u32) {
    with_method_mut(m, |dex, loc| {
        Ok(logged("access method", method_mut(dex, loc))?
            .map(reseam_apk::reseam_dex::EncodedMethod::return_early))
    });
}

#[export]
pub fn return_early_int(m: u32, value: i32) {
    with_method_mut(m, |dex, loc| {
        Ok(logged("access method", method_mut(dex, loc))?
            .map(|method| method.return_early_int(value)))
    });
}

#[export]
pub fn return_early_object_null(m: u32) {
    with_method_mut(m, |dex, loc| {
        Ok(logged("access method", method_mut(dex, loc))?
            .map(|method| method.return_early_object(0)))
    });
}

#[export]
pub fn return_early_wide(m: u32, value: i64) {
    with_method_mut(m, |dex, loc| {
        Ok(logged("access method", method_mut(dex, loc))?
            .map(|method| method.return_early_wide(value)))
    });
}
