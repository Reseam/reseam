// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::encoding::leb128::{write_sleb128, write_uleb128};
use crate::error::Result;
use crate::instruction::codec::encode_instructions;
use crate::model::code::CodeItem;
use crate::write::DexWriter;
use crate::write::sink::DexSink;

pub(super) fn write_code_item<S: DexSink>(w: &mut DexWriter<S>, code: &CodeItem) -> Result<()> {
    w.write_u16(code.registers_size);
    w.write_u16(code.ins_size);
    w.write_u16(code.compute_outs_size());
    w.write_u16(
        u16::try_from(code.tries.len())
            .map_err(|_| crate::error::invalid("code item", "more than 65535 try ranges"))?,
    );
    w.write_u32(0);
    let insns = encode_instructions(&code.instructions)?;
    w.write_u32(insns.len() as u32);
    for unit in &insns {
        w.write_u16(*unit);
    }

    if !code.tries.is_empty() {
        if !insns.len().is_multiple_of(2) {
            w.write_u16(0);
        }

        let mut handler_buf: Vec<u8> = Vec::new();
        write_uleb128(&mut handler_buf, code.catch_handlers.len() as u32);
        let mut handler_byte_offsets = Vec::new();
        for handler in &code.catch_handlers {
            handler_byte_offsets.push(handler_buf.len());
            let size = if handler.catch_all_addr.is_some() {
                -(handler.typed_catches.len() as i32)
            } else {
                handler.typed_catches.len() as i32
            };
            write_sleb128(&mut handler_buf, size);
            for tc in &handler.typed_catches {
                write_uleb128(&mut handler_buf, tc.exception_type.0);
                write_uleb128(&mut handler_buf, tc.addr);
            }
            if let Some(addr) = handler.catch_all_addr {
                write_uleb128(&mut handler_buf, addr);
            }
        }

        for t in &code.tries {
            w.write_u32(t.start_addr);
            w.write_u16(t.insn_count);
            let offset = handler_byte_offsets.get(t.handler_idx).ok_or_else(|| {
                crate::error::invalid("catch handler", "try refers to an absent handler")
            })?;
            w.write_u16(u16::try_from(*offset).map_err(|_| {
                crate::error::invalid(
                    "catch handler",
                    "referenced handler offset exceeds 65535 bytes",
                )
            })?);
        }

        w.write(&handler_buf);
    }
    Ok(())
}
