// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::cmp::Ordering;

/// Compare two strings using DEX string sort order (UTF-16 code unit comparison).
pub fn dex_string_compare(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

pub use crate::encoding::mutf8::mutf8_units;

/// DEX string sort order applied directly to MUTF-8 payloads, so surrogate
/// units compare as written rather than through a lossy scalar decode.
pub fn mutf8_compare(a: &[u8], b: &[u8]) -> Ordering {
    mutf8_units(a).cmp(mutf8_units(b))
}
