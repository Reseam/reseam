// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_dex::{AccessFlags, DexFile};

use super::logged;
use crate::bridge::trace;
use crate::context::Moved;

pub(super) fn prepare_flag_frame(
    handle: u32,
    dex: &DexFile,
    method: reseam_dex::MethodIdx,
    previous: AccessFlags,
    flags: AccessFlags,
    code: &mut Option<reseam_dex::CodeItem>,
) -> Result<Option<crate::bridge::dex::types::MethodEdit>, String> {
    if flags.intersects(AccessFlags::ABSTRACT | AccessFlags::NATIVE) {
        *code = None;
        trace::code_removed(handle);
        return Ok(None);
    }
    let Some(code) = code.as_mut() else {
        return Ok(None);
    };
    let proto = dex.proto_descriptor(&dex.proto(dex.method_id(method).proto));
    let incoming = logged(
        "change method frame",
        super::super::registers::incoming_words(&proto, flags),
    )?;
    let growth = incoming.saturating_sub(code.registers_size());
    let mapping = if growth == 0 {
        None
    } else {
        let types = dex.incoming_types(method, previous);
        let base = code.registers_size() - code.ins_size();
        let map = logged(
            "grow receiver frame",
            reseam_dex::grow_registers(code, growth, &types, &[], dex),
        )?;
        trace::moved(handle, &map, Moved::Rewrite);
        Some(super::super::mutation::growth_mapping(base, growth, map)?)
    };
    logged(
        "change method frame",
        code.set_register_frame(code.registers_size(), incoming, code.outs_size()),
    )?;
    Ok(mapping)
}
