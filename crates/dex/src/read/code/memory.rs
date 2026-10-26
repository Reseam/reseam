// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::instruction::Instruction;
use crate::model::TypeIdx;
use crate::read::u16_at;

pub fn decode_35c_type(buf: &[u8], off: usize) -> crate::Result<Instruction> {
    let unit0 = u16_at(buf, off);
    let count = ((unit0 >> 12) & 0xF) as u8;
    let type_idx = TypeIdx(u32::from(u16_at(buf, off + 2)));
    let reg_unit = u16_at(buf, off + 4);
    let args = super::invoke::decode_35c_args(count, reg_unit, unit0, off)?;
    Ok(Instruction::FilledNewArray {
        type_: type_idx,
        args,
    })
}
