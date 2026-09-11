// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Register frame queries and the one frame mutation, growing locals.

use boltffi::export;
use reseam_apk::reseam_dex::{self as dex, AccessFlags};

use crate::kotlin::handles::{code_mut, method_mut, with_code, with_method_mut};

#[export]
pub fn ensure_outs_size(m: u32, min_outs_size: u16) {
    with_method_mut(m, |dex, loc| {
        let code = code_mut(dex, loc)?;
        code.outs_size = code.outs_size.max(min_outs_size);
        Some(())
    });
}

/// Adds locals below the incoming registers and returns relocated instruction
/// indices, including the end boundary. The method is unchanged on failure.
#[export]
pub fn grow_local_registers(m: u32, additional_locals: u16) -> Option<Vec<u32>> {
    with_method_mut(m, |dex, loc| {
        let method = method_mut(dex, loc)?.clone();
        let id = dex.method_id(method.method);
        let proto = dex.proto(id.proto);
        let mut incoming = Vec::new();
        if !method.access_flags.contains(AccessFlags::STATIC) {
            incoming.push(dex.type_descriptor(id.class).into_owned());
        }
        incoming.extend(
            proto
                .parameters
                .iter()
                .map(|param| dex.type_descriptor(*param).into_owned()),
        );
        let mut code = method.code?;
        let indices = match dex::types::register_allocation::grow_registers(
            &mut code,
            additional_locals,
            &incoming,
            dex,
        ) {
            Ok(indices) => indices,
            Err(error) => {
                tracing::warn!(%error, "cannot grow method registers");
                return None;
            }
        };
        *code_mut(dex, loc)? = code;
        Some(indices.into_iter().map(|index| index as u32).collect())
    })
}

#[export]
pub fn registers_size(m: u32) -> u16 {
    with_code(m, |_, code| Some(code.registers_size)).unwrap_or(0)
}

#[export]
pub fn ins_size(m: u32) -> u16 {
    with_code(m, |_, code| Some(code.ins_size)).unwrap_or(0)
}

#[export]
pub fn outs_size(m: u32) -> u16 {
    with_code(m, |_, code| Some(code.outs_size)).unwrap_or(0)
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

/// The `position`th register operand of the instruction at `index`, or 0.
#[export]
pub fn instruction_register(m: u32, index: u32, position: u32) -> u16 {
    with_code(m, |_, code| {
        code.instructions
            .get(index as usize)?
            .registers_used()
            .get(position as usize)
            .copied()
    })
    .unwrap_or(0)
}

#[export]
pub fn instruction_wide_literal(m: u32, index: u32) -> i64 {
    with_code(m, |_, code| {
        code.instructions.get(index as usize)?.literal()
    })
    .unwrap_or(0)
}
