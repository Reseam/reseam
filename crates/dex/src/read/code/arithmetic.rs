// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::read::u16_at;

pub fn decode_23x(buf: &[u8], off: usize) -> [u8; 3] {
    let aa = (u16_at(buf, off) >> 8) as u8;
    let unit1 = u16_at(buf, off + 2);
    let bb = unit1 as u8;
    let cc = (unit1 >> 8) as u8;
    [aa, bb, cc]
}
