// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

pub(super) const LANGUAGE_BASE: u8 = b'a';
pub(super) const REGION_BASE: u8 = b'0';

/// Two-character codes are stored as they are; three-character codes are packed into five bits
/// each with the high bit set, as `ResTable_config::packLanguageOrRegion` does.
pub(super) fn pack(code: &[u8], base: u8) -> [u8; 2] {
    match *code {
        [first, second] => [first, second],
        [first, second, third] => {
            let (first, second, third) = (first - base, second - base, third - base);
            [0x80 | (third << 2) | (second >> 3), (second << 5) | first]
        }
        _ => unreachable!("language and region codes have two or three characters"),
    }
}

pub(super) fn unpack(first: u8, second: u8, base: u8) -> String {
    if first & 0x80 == 0 {
        return [first, second]
            .into_iter()
            .take_while(|&byte| byte != 0)
            .map(char::from)
            .collect();
    }
    [
        second & 0x1f,
        ((second & 0xe0) >> 5) | ((first & 0x03) << 3),
        (first & 0x7c) >> 2,
    ]
    .into_iter()
    .map(|letter| char::from(base + letter))
    .collect()
}
