// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::handlers::read_tries_and_handlers;
use crate::error::Result;
use crate::instruction::codec::decode_instructions;
use crate::model::code::CodeItem;
use crate::options::ParseOptions;
use reseam_bytes::{u16_at, u32_at};

use crate::model::metadata::Metadata;

/// Decodes one `code_item`, including debug info and exception metadata.
pub fn read_code_item(source: &crate::DexBytes, off: u32, opts: ParseOptions) -> Result<CodeItem> {
    let buf = source.as_bytes();
    let base = off as usize;
    reseam_bytes::require_len(buf, base, 16, "code item")?;
    let registers_size = u16_at(buf, base);
    let ins_size = u16_at(buf, base + 2);
    let outs_size = u16_at(buf, base + 4);
    let tries_size = u16_at(buf, base + 6);
    let debug_info_off = u32_at(buf, base + 8);
    let insns_size = u32_at(buf, base + 12) as usize;

    let insns_start = base + 16;
    let instructions = decode_instructions(buf, insns_start, insns_size)?;

    let debug_info = (debug_info_off != 0)
        .then(|| Metadata::from_source(source, debug_info_off, opts, opts.debug_info))
        .transpose()?;

    let (tries, catch_handlers) = if tries_size > 0 {
        let mut tries_off = insns_start + insns_size * 2;
        if !insns_size.is_multiple_of(2) {
            tries_off += 2;
        }
        read_tries_and_handlers(buf, tries_off, tries_size, opts)?
    } else {
        (Vec::new(), Vec::new())
    };

    Ok(CodeItem {
        registers_size,
        ins_size,
        outs_size,
        debug_info,
        instructions,
        tries,
        catch_handlers,
    })
}
