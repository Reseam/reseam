// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::RewriteLayout;
use crate::error::{Result, invalid};
use crate::instruction::branch::Addresses;
use crate::model::code::{CatchHandler, CodeItem, TryItem};
use crate::model::debug::{DebugBytecode, DebugInfo};
use crate::model::metadata::Metadata as Lazy;

/// A body's exception ranges and debug positions moved to a new layout.
pub(super) struct Metadata {
    pub(super) tries: Vec<TryItem>,
    pub(super) handlers: Vec<CatchHandler>,
    pub(super) debug_info: Option<Lazy<DebugInfo>>,
}

impl Metadata {
    /// The metadata of `code` with every address in `original` coordinates
    /// moved to where `layout` puts it. Exception addresses must be
    /// instruction boundaries; a debug address inside an instruction keeps
    /// its offset into that instruction's new place.
    pub(super) fn relocated(
        code: &CodeItem,
        original: &Addresses,
        layout: &RewriteLayout,
    ) -> Result<Self> {
        let relocate = |address| {
            original
                .index_of(address)
                .map(|index| layout.starts[index])
                .ok_or_else(|| {
                    invalid(
                        "instruction rewrite",
                        format!(
                            "metadata address {address} is not an instruction boundary (code ends at {})",
                            original.last().expect("addresses include the end boundary")
                        ),
                    )
                })
        };
        let mut tries = code.tries.clone();
        for protected in &mut tries {
            let old_end = protected
                .start_addr
                .checked_add(u32::from(protected.insn_count))
                .ok_or_else(|| invalid("instruction rewrite", "try range overflow"))?;
            let end = relocate(old_end)?;
            protected.start_addr = relocate(protected.start_addr)?;
            protected.insn_count = u16::try_from(end - protected.start_addr).map_err(|_| {
                invalid(
                    "instruction rewrite",
                    "expanded try range exceeds 65535 code units",
                )
            })?;
        }
        let mut handlers = code.catch_handlers.clone();
        for handler in &mut handlers {
            for catch in &mut handler.typed_catches {
                catch.addr = relocate(catch.addr)?;
            }
            handler.catch_all_addr = handler.catch_all_addr.map(relocate).transpose()?;
        }
        let debug_info = code
            .debug_info
            .as_ref()
            .map(|metadata| {
                let mut metadata = metadata.clone();
                let debug = metadata.resolve_mut()?;
                *debug = relocate_debug(debug, &|address| {
                    relocate_debug_address(original, layout, address)
                })?;
                Ok::<_, crate::DexError>(metadata)
            })
            .transpose()?;
        Ok(Self {
            tries,
            handlers,
            debug_info,
        })
    }
}

fn relocate_debug_address(
    original: &Addresses,
    layout: &RewriteLayout,
    address: u32,
) -> Result<u32> {
    let index = original.partition_point(|&boundary| boundary <= address) - 1;
    let old_start = original[index];
    let new_start = if address == old_start {
        layout.starts[index]
    } else {
        layout.bodies.get(index).copied().unwrap_or(layout.end)
    };
    let within = address - old_start;
    let within = if let Some(&next) = layout.starts.get(index + 1) {
        within.min(next.saturating_sub(new_start))
    } else {
        within
    };
    new_start
        .checked_add(within)
        .ok_or_else(|| invalid("debug relocation", "address overflow"))
}

fn relocate_debug(debug: &DebugInfo, relocate: &impl Fn(u32) -> Result<u32>) -> Result<DebugInfo> {
    let mut debug = debug.clone();
    let mut old_address = 0u32;
    let mut new_address = relocate(0)?;
    for bytecode in &mut debug.bytecodes {
        let advance = match bytecode {
            DebugBytecode::AdvancePc { advance } => advance,
            DebugBytecode::SpecialAdvance { pc_advance, .. } => pc_advance,
            _ => continue,
        };
        old_address = old_address
            .checked_add(*advance)
            .ok_or_else(|| invalid("debug relocation", "address overflow"))?;
        let address = relocate(old_address)?;
        *advance = address
            .checked_sub(new_address)
            .ok_or_else(|| invalid("debug relocation", "address moved backwards"))?;
        new_address = address;
    }
    if relocate(0)? != 0 {
        debug.bytecodes.insert(
            0,
            DebugBytecode::AdvancePc {
                advance: relocate(0)?,
            },
        );
    }
    Ok(debug)
}
