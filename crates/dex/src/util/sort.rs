// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::cmp::Ordering;

pub use crate::encoding::mutf8::mutf8_units;

/// DEX string sort order applied directly to MUTF-8 payloads, so surrogate
/// units compare as written rather than through a lossy scalar decode.
pub fn mutf8_compare(a: &[u8], b: &[u8]) -> Ordering {
    mutf8_units(a).cmp(mutf8_units(b))
}

/// The DEX sort order of two MUTF-8 payloads, each ending at a NUL byte or the
/// end of its slice, when it follows from a common ASCII prefix: there each byte
/// is one code unit. `None` once either side reaches a non-ASCII byte.
pub fn ascii_order(a: &[u8], b: &[u8]) -> Option<Ordering> {
    let unit = |payload: &[u8], at: usize| payload.get(at).copied().filter(|&byte| byte != 0);
    let mut at = 0;
    loop {
        let (left, right) = (unit(a, at), unit(b, at));
        if left.max(right).is_some_and(|byte| byte >= 0x80) {
            return None;
        }
        if left != right || left.is_none() {
            return Some(left.cmp(&right));
        }
        at += 1;
    }
}
