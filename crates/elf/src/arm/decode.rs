// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::instruction::Instruction;
use super::mode::Mode;
use super::{a32, t32};

/// The instruction in `mode` at `address` and its size in bytes, from the code section `code`
/// loaded at `start`. `None` when the bytes run out. An undefined encoding traps, so it decodes
/// as `Flow::Stop`.
pub(super) fn read(
    code: &[u8],
    start: u64,
    mode: Mode,
    address: u64,
) -> Option<(Instruction, u64)> {
    let at = usize::try_from(address.checked_sub(start)?).ok()?;
    let halfword = |index: usize| {
        let bytes = code.get(at + 2 * index..at + 2 * index + 2)?;
        Some(u32::from(u16::from_le_bytes([bytes[0], bytes[1]])))
    };
    let first = halfword(0)?;
    Some(match mode {
        Mode::Arm => (a32::decode(first | halfword(1)? << 16, address), 4),
        Mode::Thumb if t32::is_wide(first) => (t32::decode_wide(first, halfword(1)?, address), 4),
        Mode::Thumb => (t32::decode_narrow(first, address), 2),
    })
}
