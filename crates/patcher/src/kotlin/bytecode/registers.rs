// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::kotlin::handles::record_failure;
use boltffi::export;
use reseam_apk::reseam_dex::{self as dex, AccessFlags};

use crate::kotlin::handles::{code_mut, method_mut, with_code, with_instruction, with_method_mut};
use crate::kotlin::types::{MethodEdit, RegisterWriters};

use super::logged;

pub(super) fn incoming_types(
    dex: &dex::DexFile,
    method: dex::MethodIdx,
    flags: AccessFlags,
) -> Vec<dex::TypeIdx> {
    let id = dex.method_id(method);
    let proto = dex.proto(id.proto);
    let mut incoming = Vec::with_capacity(proto.parameters.len() + 1);
    if !flags.contains(AccessFlags::STATIC) {
        incoming.push(id.class);
    }
    incoming.extend(proto.parameters);
    incoming
}

#[export]
pub fn ensure_outs_size(m: u32, min_outs_size: u16) {
    with_method_mut(m, |dex, loc| {
        let code = logged("ensure outgoing registers", code_mut(dex, loc))?
            .ok_or_else(|| format!("method handle {m} has no code"))?;
        code.ensure_outs_size(min_outs_size);
        Ok(Some(()))
    });
}

#[export]
pub fn grow_local_registers(
    m: u32,
    additional_locals: u16,
    protected: Vec<u16>,
) -> Result<MethodEdit, String> {
    let outcome = with_method_mut(m, |dex, loc| {
        let method = logged("access method", method_mut(dex, loc))?
            .ok_or_else(|| format!("method handle {m} is missing"))?;
        let (id, flags) = (method.method, method.access_flags);
        let mut code = method
            .code
            .take()
            .ok_or_else(|| format!("method handle {m} has no code"))?;
        let incoming = incoming_types(dex, id, flags);
        let indices = logged(
            "grow method registers",
            dex::types::register_allocation::grow_registers(
                &mut code,
                additional_locals,
                &incoming,
                &protected,
                dex,
            ),
        );
        logged("restore method code", method_mut(dex, loc))?
            .expect("frame growth leaves the method slot in place")
            .code = Some(code);
        let mut mapping = super::mutation::edit_mapping(indices?)?;
        mapping.register_shift = additional_locals;
        Ok(Some(mapping))
    });
    crate::kotlin::handles::check_call()?;
    outcome.ok_or_else(|| format!("method handle {m} could not grow"))
}

#[export]
pub fn registers_size(m: u32) -> u16 {
    with_code(m, |_, code| Some(code.registers_size())).unwrap_or(0)
}

#[export]
pub fn ins_size(m: u32) -> u16 {
    with_code(m, |_, code| Some(code.ins_size())).unwrap_or(0)
}

#[export]
pub fn outs_size(m: u32) -> u16 {
    with_code(m, |_, code| Some(code.outs_size())).unwrap_or(0)
}

#[export]
pub fn find_free_register(m: u32, at_index: u32, exclude: Vec<u16>) -> Option<u16> {
    with_code(m, |_, code| {
        dex::find_free_register(code, at_index as usize, &exclude)
    })
}

#[export]
pub fn find_free_registers(m: u32, at_index: u32, count: u32, exclude: Vec<u16>) -> Vec<u16> {
    with_code(m, |_, code| {
        dex::find_free_registers(code, at_index as usize, count as usize, &exclude)
    })
    .unwrap_or_default()
}

#[export]
pub fn find_contiguous_free_registers(
    m: u32,
    at_index: u32,
    count: u32,
    exclude: Vec<u16>,
) -> Vec<u16> {
    with_code(m, |_, code| {
        dex::find_contiguous_free_registers(code, at_index as usize, count as usize, &exclude)
    })
    .unwrap_or_default()
}

#[export]
pub fn register_writers(m: u32, index: u32, register: u16) -> Option<RegisterWriters> {
    with_code(m, |_, code| {
        let (indices, from_entry) = dex::reaching_definitions(code, index as usize, register)?;
        Some(RegisterWriters {
            indices: indices.into_iter().map(|index| index as u32).collect(),
            from_entry,
        })
    })
}

#[export]
pub fn instruction_register(m: u32, index: u32, position: u32) -> u16 {
    with_instruction(m, index, |_, instruction| {
        instruction.registers_used().get(position as usize).copied()
    })
    .unwrap_or_else(|| {
        record_failure(format!(
            "method {m} instruction {index} has no register operand {position}"
        ));
        0
    })
}

#[export]
pub fn instruction_wide_literal(m: u32, index: u32) -> i64 {
    with_instruction(m, index, |_, instruction| instruction.literal()).unwrap_or_else(|| {
        record_failure(format!("method {m} instruction {index} has no literal"));
        0
    })
}
