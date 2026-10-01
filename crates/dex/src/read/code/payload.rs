// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::encoding::leb128::{read_sleb128_with_opts, read_uleb128_with_opts};
use crate::error::{Result, invalid_offset, require_len};
use crate::read::{u16_at, u32_at};
use crate::types::TypeIdx;
use crate::types::code::{CatchHandler, TryItem, TypedCatch};
use crate::types::header::ParseOptions;

pub(crate) enum HandlerEvent {
    Count(u32),
    Handler { offset: usize, size: i32 },
    Typed(TypedCatch),
    CatchAll(u32),
}

pub(crate) fn walk_handler_list(
    buf: &[u8],
    list_off: usize,
    opts: ParseOptions,
    mut visit: impl FnMut(HandlerEvent) -> Result<()>,
) -> Result<usize> {
    let (count, size) = read_uleb128_with_opts(buf, list_off, opts)?;
    visit(HandlerEvent::Count(count))?;
    let mut pos = list_off + size;
    for _ in 0..count {
        let offset = pos - list_off;
        let (size, consumed) = read_sleb128_with_opts(buf, pos, opts)?;
        pos += consumed;
        visit(HandlerEvent::Handler { offset, size })?;
        for _ in 0..size.unsigned_abs() {
            let (type_idx, consumed) = read_uleb128_with_opts(buf, pos, opts)?;
            pos += consumed;
            let (addr, consumed) = read_uleb128_with_opts(buf, pos, opts)?;
            pos += consumed;
            visit(HandlerEvent::Typed(TypedCatch {
                exception_type: TypeIdx(type_idx),
                addr,
            }))?;
        }
        if size <= 0 {
            let (addr, consumed) = read_uleb128_with_opts(buf, pos, opts)?;
            pos += consumed;
            visit(HandlerEvent::CatchAll(addr))?;
        }
    }
    Ok(pos)
}

pub(crate) fn handler_index(offsets: &[usize], offset: usize) -> Result<usize> {
    offsets
        .binary_search(&offset)
        .map_err(|_| invalid_offset("catch handler", offset as u32, 0))
}

pub fn read_tries_and_handlers(
    buf: &[u8],
    tries_off: usize,
    tries_size: u16,
    opts: ParseOptions,
) -> Result<(Vec<TryItem>, Vec<CatchHandler>)> {
    require_len(buf, tries_off, usize::from(tries_size) * 8, "try items")?;
    let list_off = tries_off + usize::from(tries_size) * 8;
    let mut offsets = Vec::new();
    let mut handlers: Vec<CatchHandler> = Vec::new();
    walk_handler_list(buf, list_off, opts, |event| {
        match event {
            HandlerEvent::Count(_) => {}
            HandlerEvent::Handler { offset, .. } => {
                offsets.push(offset);
                handlers.push(CatchHandler {
                    typed_catches: Vec::new(),
                    catch_all_addr: None,
                });
            }
            HandlerEvent::Typed(catch) => handlers
                .last_mut()
                .expect("typed catches follow a handler header")
                .typed_catches
                .push(catch),
            HandlerEvent::CatchAll(addr) => {
                handlers
                    .last_mut()
                    .expect("catch-all follows a handler header")
                    .catch_all_addr = Some(addr);
            }
        }
        Ok(())
    })?;
    let tries = (0..usize::from(tries_size))
        .map(|index| {
            let off = tries_off + index * 8;
            Ok(TryItem {
                start_addr: u32_at(buf, off),
                insn_count: u16_at(buf, off + 4),
                handler_idx: handler_index(&offsets, usize::from(u16_at(buf, off + 6)))?,
            })
        })
        .collect::<Result<_>>()?;
    Ok((tries, handlers))
}
