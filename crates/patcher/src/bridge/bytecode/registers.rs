// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;

use boltffi::export;
use reseam_dex::{self as dex, AccessFlags};

use crate::bridge::dex::types::{MethodEdit, RegisterWriters};
use crate::bridge::handles::{with_code, with_method_mut};
use crate::context::{code_mut, method_mut};

use super::logged;

/// The register words a method with prototype `proto` and these flags is entered with.
pub(super) fn incoming_words(proto: &str, flags: AccessFlags) -> dex::Result<u16> {
    let receiver = if flags.contains(AccessFlags::STATIC) {
        dex::Receiver::Absent
    } else {
        dex::Receiver::Present
    };
    let words: u32 = dex::argument_kinds(proto, receiver)?
        .into_iter()
        .map(|kind| u32::from(kind.words()))
        .sum();
    u16::try_from(words).map_err(|_| dex::DexError::Invalid {
        section: "invoke",
        reason: "incoming register words exceed 65535".into(),
    })
}

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
            .map(Arc::unwrap_or_clone)
            .ok_or_else(|| format!("method handle {m} has no code"))?;
        let incoming = incoming_types(dex, id, flags);
        let base = code.registers_size() - code.ins_size();
        let indices = logged(
            "grow method registers",
            dex::grow_registers(&mut code, additional_locals, &incoming, &protected, dex),
        );
        logged("restore method code", method_mut(dex, loc))?
            .expect("frame growth leaves the method slot in place")
            .code = Some(Arc::new(code));
        super::mutation::growth_mapping(base, additional_locals, indices?).map(Some)
    });
    crate::bridge::run_state::check_call()?;
    outcome.ok_or_else(|| format!("method handle {m} could not grow"))
}

#[export]
pub fn find_free_register(m: u32, at_index: u32, exclude: Vec<u16>) -> Option<u16> {
    with_code(m, |_, code| {
        dex::find_free_register(code, at_index as usize, &exclude)
    })
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
