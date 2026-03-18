use crate::types::instruction::Instruction;
use crate::types::TypeIdx;

use super::format::u16_at;

pub fn decode_35c_type(buf: &[u8], off: usize) -> Instruction {
    let unit0 = u16_at(buf, off);
    let count = ((unit0 >> 12) & 0xF) as u8;
    let type_idx = TypeIdx(u16_at(buf, off + 2) as u32);
    let reg_unit = u16_at(buf, off + 4);
    let args = super::invoke::decode_35c_args(count, reg_unit, unit0);
    Instruction::FilledNewArray {
        type_: type_idx,
        args,
    }
}
