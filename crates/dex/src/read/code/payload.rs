use crate::encoding::leb128::{read_sleb128, read_uleb128};
use crate::error::{invalid_offset, Result};
use crate::types::code::{CatchHandler, TryItem, TypedCatch};
use crate::types::TypeIdx;

use super::format::{u16_at, u32_at};

/// Decodes the try-item table and the shared encoded catch-handler list.
pub fn read_tries_and_handlers(
    buf: &[u8],
    tries_off: usize,
    tries_size: u16,
) -> Result<(Vec<TryItem>, Vec<CatchHandler>)> {
    let mut tries = Vec::with_capacity(tries_size as usize);
    let handler_list_off = tries_off + tries_size as usize * 8;

    let (handler_count, n) = read_uleb128(buf, handler_list_off)?;
    let mut pos = handler_list_off + n;
    let mut handler_offsets: Vec<usize> = Vec::new();
    let mut catch_handlers = Vec::with_capacity(handler_count as usize);

    for _ in 0..handler_count {
        handler_offsets.push(pos - handler_list_off);

        let (size_raw, n) = read_sleb128(buf, pos)?;
        pos += n;

        let catch_count = size_raw.unsigned_abs() as usize;
        let has_catch_all = size_raw <= 0;

        let mut typed_catches = Vec::with_capacity(catch_count);
        for _ in 0..catch_count {
            let (type_idx, n) = read_uleb128(buf, pos)?;
            pos += n;
            let (addr, n) = read_uleb128(buf, pos)?;
            pos += n;
            typed_catches.push(TypedCatch {
                exception_type: TypeIdx(type_idx),
                addr,
            });
        }

        let catch_all_addr = if has_catch_all {
            let (addr, n) = read_uleb128(buf, pos)?;
            pos += n;
            Some(addr)
        } else {
            None
        };

        catch_handlers.push(CatchHandler {
            typed_catches,
            catch_all_addr,
        });
    }

    for i in 0..tries_size as usize {
        let t_off = tries_off + i * 8;
        crate::error::require_len(buf, t_off, 8, "try item")?;
        let start_addr = u32_at(buf, t_off);
        let insn_count = u16_at(buf, t_off + 4);
        let handler_off = u16_at(buf, t_off + 6) as usize;

        let handler_idx = handler_offsets
            .iter()
            .position(|&o| o == handler_off)
            .ok_or_else(|| invalid_offset("catch_handler", handler_off as u32, buf.len() as u32))?;

        tries.push(TryItem {
            start_addr,
            insn_count,
            handler_idx,
        });
    }

    Ok((tries, catch_handlers))
}
