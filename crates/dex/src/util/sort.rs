// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::cmp::Ordering;

/// Compare two strings using DEX string sort order (UTF-16 code unit comparison).
pub fn dex_string_compare(a: &str, b: &str) -> Ordering {
    let mut a_units = a.encode_utf16();
    let mut b_units = b.encode_utf16();
    loop {
        match (a_units.next(), b_units.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(au), Some(bu)) => {
                let cmp = au.cmp(&bu);
                if cmp != Ordering::Equal {
                    return cmp;
                }
            }
        }
    }
}

/// UTF-16 code units of a MUTF-8 payload, surrogate halves kept as their own
/// unit. A three-byte group decodes to one unit whether it is a BMP scalar or a
/// surrogate (CESU-8), which is exactly the sequence a decoder reads back and
/// the order DEX sorts by. A malformed lead byte falls back to itself so the
/// order stays total.
pub fn mutf8_units(bytes: &[u8]) -> impl Iterator<Item = u16> + '_ {
    let mut i = 0;
    std::iter::from_fn(move || {
        let b = *bytes.get(i)?;
        let (unit, len) = if b < 0x80 {
            (b as u16, 1)
        } else if b & 0xE0 == 0xC0 {
            let n1 = *bytes.get(i + 1).unwrap_or(&0) as u16;
            ((((b & 0x1F) as u16) << 6) | (n1 & 0x3F), 2)
        } else if b & 0xF0 == 0xE0 {
            let n1 = *bytes.get(i + 1).unwrap_or(&0) as u16;
            let n2 = *bytes.get(i + 2).unwrap_or(&0) as u16;
            (
                (((b & 0x0F) as u16) << 12) | ((n1 & 0x3F) << 6) | (n2 & 0x3F),
                3,
            )
        } else {
            (b as u16, 1)
        };
        i += len;
        Some(unit)
    })
}

/// DEX string sort order applied directly to MUTF-8 payloads, so surrogate
/// units compare as written rather than through a lossy scalar decode.
pub fn mutf8_compare(a: &[u8], b: &[u8]) -> Ordering {
    mutf8_units(a).cmp(mutf8_units(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_sort() {
        assert_eq!(dex_string_compare("abc", "abd"), Ordering::Less);
        assert_eq!(dex_string_compare("abc", "abc"), Ordering::Equal);
        assert_eq!(dex_string_compare("abd", "abc"), Ordering::Greater);
    }

    #[test]
    fn test_prefix_sort() {
        assert_eq!(dex_string_compare("ab", "abc"), Ordering::Less);
    }

    #[test]
    fn mutf8_units_keep_surrogates() {
        // Supplementary character U+1D11E is CESU-8 ed a0 b4 ed b4 9e in MUTF-8:
        // the high surrogate 0xD834 then the low 0xDD1E, not one scalar.
        let bytes = [0xed, 0xa0, 0xb4, 0xed, 0xb4, 0x9e];
        assert_eq!(mutf8_units(&bytes).collect::<Vec<_>>(), [0xD834, 0xDD1E]);
    }

    #[test]
    fn mutf8_orders_surrogate_below_high_bmp() {
        // Two obfuscated strings from a real APK that a scalar-decoding sort put
        // out of order: they share a prefix, then one has 0xEB07 (ee ac 87) and
        // the other 0xDC9F (ed b2 9f). The surrogate unit 0xDC9F is the smaller
        // UTF-16 unit, so its string must sort first.
        let prefix = "ea9ea6ef8886ef8491e99e8ee5b4aae59aa0";
        let hex = |s: &str| {
            (0..s.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
                .collect::<Vec<u8>>()
        };
        let with_high_bmp = hex(&format!("{prefix}eeac87"));
        let with_surrogate = hex(&format!("{prefix}edb29f"));
        assert_eq!(
            mutf8_compare(&with_surrogate, &with_high_bmp),
            Ordering::Less
        );
        // A lone lead byte with no continuation still yields a total order.
        assert_eq!(mutf8_compare(&[0xed], &[0xee]), Ordering::Less);
    }
}
