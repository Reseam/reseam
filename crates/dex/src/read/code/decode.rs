use crate::error::{malformed, require_len, Result};
use crate::types::instruction::Instruction;

use super::arithmetic::decode_23x;
use super::format::{i32_at, u16_at, u32_at};
use super::invoke::{decode_35c_invoke, decode_3rc_invoke, decode_invoke_polymorphic};
use super::memory::decode_35c_type;

fn nibbles(unit: u16) -> (u8, u8) {
    let a = ((unit >> 8) & 0xF) as u8;
    let b = ((unit >> 12) & 0xF) as u8;
    (a, b)
}

fn hi8(unit: u16) -> u8 {
    (unit >> 8) as u8
}

fn min_instruction_bytes(opcode: u8) -> usize {
    match opcode {
        0x03
        | 0x06
        | 0x09
        | 0x14
        | 0x17
        | 0x1b
        | 0x24
        | 0x25
        | 0x26
        | 0x2a
        | 0x2b
        | 0x2c
        | 0x6e..=0x72
        | 0x74..=0x78
        | 0xfc
        | 0xfd => 6,
        0x18 => 10,
        0xfa | 0xfb => 8,
        0x00
        | 0x01
        | 0x04
        | 0x07
        | 0x0a..=0x12
        | 0x1d..=0x1e
        | 0x21
        | 0x27..=0x28
        | 0x3e..=0x43
        | 0x73
        | 0x79..=0x7a
        | 0x7b..=0x8f
        | 0xb0..=0xcf
        | 0xe3..=0xf9 => 2,
        _ => 4,
    }
}

pub fn decode_instructions(
    buf: &[u8],
    start: usize,
    insns_size: usize,
) -> Result<Vec<Instruction>> {
    let mut instructions = Vec::new();
    let mut pc = 0usize;

    while pc < insns_size {
        let unit_off = start + pc * 2;
        require_len(buf, unit_off, 2, "code item instruction")?;
        let unit0 = u16_at(buf, unit_off);
        let opcode = (unit0 & 0xFF) as u8;
        require_len(
            buf,
            unit_off,
            min_instruction_bytes(opcode),
            "code item instruction",
        )?;

        let insn = match opcode {
            0x00 => {
                if unit0 == 0x0100 {
                    let size = u16_at(buf, unit_off + 2) as usize;
                    require_len(
                        buf,
                        unit_off,
                        (1 + 1 + 2 + size * 2) * 2,
                        "packed-switch payload",
                    )?;
                    let first_key = i32_at(buf, unit_off + 4);
                    let mut targets = Vec::with_capacity(size);
                    for i in 0..size {
                        targets.push(i32_at(buf, unit_off + 8 + i * 4));
                    }
                    let total_units = 1 + 1 + 2 + size * 2;
                    pc += total_units;
                    instructions.push(Instruction::PackedSwitchPayload { first_key, targets });
                    continue;
                } else if unit0 == 0x0200 {
                    let size = u16_at(buf, unit_off + 2) as usize;
                    require_len(
                        buf,
                        unit_off,
                        (1 + 1 + size * 2 + size * 2) * 2,
                        "sparse-switch payload",
                    )?;
                    let mut keys_and_targets = Vec::with_capacity(size);
                    for i in 0..size {
                        let key = i32_at(buf, unit_off + 4 + i * 4);
                        let target = i32_at(buf, unit_off + 4 + size * 4 + i * 4);
                        keys_and_targets.push((key, target));
                    }
                    let total_units = 1 + 1 + size * 2 + size * 2;
                    pc += total_units;
                    instructions.push(Instruction::SparseSwitchPayload { keys_and_targets });
                    continue;
                } else if unit0 == 0x0300 {
                    let element_width = u16_at(buf, unit_off + 2);
                    let size = u32_at(buf, unit_off + 4) as usize;
                    let data_bytes = size.checked_mul(element_width as usize).ok_or_else(|| {
                        malformed(
                            "fill-array-data payload",
                            unit_off,
                            "payload size overflowed",
                        )
                    })?;
                    require_len(buf, unit_off, 8 + data_bytes, "fill-array-data payload")?;
                    let data = buf[unit_off + 8..unit_off + 8 + data_bytes].to_vec();
                    let total_units = (8 + data_bytes).div_ceil(2);
                    pc += total_units;
                    instructions.push(Instruction::FillArrayDataPayload {
                        element_width,
                        data,
                    });
                    continue;
                }
                Instruction::Nop
            }

            0x01 => {
                let (a, b) = nibbles(unit0);
                Instruction::Move { dest: a, src: b }
            }
            0x04 => {
                let (a, b) = nibbles(unit0);
                Instruction::MoveWide { dest: a, src: b }
            }
            0x07 => {
                let (a, b) = nibbles(unit0);
                Instruction::MoveObject { dest: a, src: b }
            }
            0x21 => {
                let (a, b) = nibbles(unit0);
                Instruction::ArrayLength { dest: a, array: b }
            }

            0x02 => {
                let aa = hi8(unit0);
                let bb = u16_at(buf, unit_off + 2);
                Instruction::MoveFrom16 { dest: aa, src: bb }
            }
            0x05 => {
                let aa = hi8(unit0);
                let bb = u16_at(buf, unit_off + 2);
                Instruction::MoveWideFrom16 { dest: aa, src: bb }
            }
            0x08 => {
                let aa = hi8(unit0);
                let bb = u16_at(buf, unit_off + 2);
                Instruction::MoveObjectFrom16 { dest: aa, src: bb }
            }

            0x03 => {
                let aa = u16_at(buf, unit_off + 2);
                let bb = u16_at(buf, unit_off + 4);
                pc += 3;
                instructions.push(Instruction::Move16 { dest: aa, src: bb });
                continue;
            }
            0x06 => {
                let aa = u16_at(buf, unit_off + 2);
                let bb = u16_at(buf, unit_off + 4);
                pc += 3;
                instructions.push(Instruction::MoveWide16 { dest: aa, src: bb });
                continue;
            }
            0x09 => {
                let aa = u16_at(buf, unit_off + 2);
                let bb = u16_at(buf, unit_off + 4);
                pc += 3;
                instructions.push(Instruction::MoveObject16 { dest: aa, src: bb });
                continue;
            }

            0x0a => Instruction::MoveResult { dest: hi8(unit0) },
            0x0b => Instruction::MoveResultWide { dest: hi8(unit0) },
            0x0c => Instruction::MoveResultObject { dest: hi8(unit0) },
            0x0d => Instruction::MoveException { dest: hi8(unit0) },

            0x0e => Instruction::ReturnVoid,
            0x0f => Instruction::Return { src: hi8(unit0) },
            0x10 => Instruction::ReturnWide { src: hi8(unit0) },
            0x11 => Instruction::ReturnObject { src: hi8(unit0) },

            0x12 => {
                let (a, b) = nibbles(unit0);
                let value = ((b as i8) << 4) >> 4;
                Instruction::Const4 { dest: a, value }
            }

            0x13 => {
                let aa = hi8(unit0);
                let v = u16_at(buf, unit_off + 2) as i16;
                Instruction::Const16 { dest: aa, value: v }
            }
            0x16 => {
                let aa = hi8(unit0);
                let v = u16_at(buf, unit_off + 2) as i16;
                Instruction::ConstWide16 { dest: aa, value: v }
            }

            0x14 => {
                let aa = hi8(unit0);
                let lo = u16_at(buf, unit_off + 2) as u32;
                let hi = u16_at(buf, unit_off + 4) as u32;
                let value = (hi << 16 | lo) as i32;
                pc += 3;
                instructions.push(Instruction::Const { dest: aa, value });
                continue;
            }
            0x17 => {
                let aa = hi8(unit0);
                let lo = u16_at(buf, unit_off + 2) as u32;
                let hi = u16_at(buf, unit_off + 4) as u32;
                let value = (hi << 16 | lo) as i32;
                pc += 3;
                instructions.push(Instruction::ConstWide32 { dest: aa, value });
                continue;
            }

            0x15 => {
                let aa = hi8(unit0);
                let v = u16_at(buf, unit_off + 2) as i16;
                Instruction::ConstHigh16 { dest: aa, value: v }
            }
            0x19 => {
                let aa = hi8(unit0);
                let v = u16_at(buf, unit_off + 2) as i16;
                Instruction::ConstWideHigh16 { dest: aa, value: v }
            }

            0x18 => {
                let aa = hi8(unit0);
                let mut value: i64 = 0;
                for i in 0..4u64 {
                    value |= (u16_at(buf, unit_off + 2 + i as usize * 2) as i64) << (i * 16);
                }
                pc += 5;
                instructions.push(Instruction::ConstWide { dest: aa, value });
                continue;
            }

            0x1a => {
                let aa = hi8(unit0);
                let idx = u16_at(buf, unit_off + 2);
                Instruction::ConstString {
                    dest: aa,
                    string: crate::types::StringIdx(idx as u32),
                }
            }
            0x1c => {
                let aa = hi8(unit0);
                let idx = u16_at(buf, unit_off + 2);
                Instruction::ConstClass {
                    dest: aa,
                    type_: crate::types::TypeIdx(idx as u32),
                }
            }
            0xfe => {
                let aa = hi8(unit0);
                let idx = u16_at(buf, unit_off + 2);
                Instruction::ConstMethodHandle {
                    dest: aa,
                    method_handle: crate::types::method_handle::MethodHandleIdx(idx as u32),
                }
            }
            0xff => {
                let aa = hi8(unit0);
                let idx = u16_at(buf, unit_off + 2);
                Instruction::ConstMethodType {
                    dest: aa,
                    proto: crate::types::ProtoIdx(idx),
                }
            }

            0x1b => {
                let aa = hi8(unit0);
                let lo = u16_at(buf, unit_off + 2) as u32;
                let hi = u16_at(buf, unit_off + 4) as u32;
                let idx = hi << 16 | lo;
                pc += 3;
                instructions.push(Instruction::ConstStringJumbo {
                    dest: aa,
                    string: crate::types::StringIdx(idx),
                });
                continue;
            }

            0x1d => Instruction::MonitorEnter { ref_: hi8(unit0) },
            0x1e => Instruction::MonitorExit { ref_: hi8(unit0) },

            0x1f => {
                let aa = hi8(unit0);
                let idx = u16_at(buf, unit_off + 2);
                Instruction::CheckCast {
                    ref_: aa,
                    type_: crate::types::TypeIdx(idx as u32),
                }
            }

            0x20 => {
                let (a, b) = nibbles(unit0);
                let idx = u16_at(buf, unit_off + 2);
                Instruction::InstanceOf {
                    dest: a,
                    ref_: b,
                    type_: crate::types::TypeIdx(idx as u32),
                }
            }

            0x22 => {
                let aa = hi8(unit0);
                let idx = u16_at(buf, unit_off + 2);
                Instruction::NewInstance {
                    dest: aa,
                    type_: crate::types::TypeIdx(idx as u32),
                }
            }

            0x23 => {
                let (a, b) = nibbles(unit0);
                let idx = u16_at(buf, unit_off + 2);
                Instruction::NewArray {
                    dest: a,
                    size: b,
                    type_: crate::types::TypeIdx(idx as u32),
                }
            }

            0x24 => {
                let insn = decode_35c_type(buf, unit_off);
                pc += 3;
                instructions.push(insn);
                continue;
            }

            0x25 => {
                let count = hi8(unit0);
                let type_idx = crate::types::TypeIdx(u16_at(buf, unit_off + 2) as u32);
                let first_reg = u16_at(buf, unit_off + 4);
                pc += 3;
                instructions.push(Instruction::FilledNewArrayRange {
                    type_: type_idx,
                    first_reg,
                    count,
                });
                continue;
            }

            0x26 => {
                let aa = hi8(unit0);
                let lo = u16_at(buf, unit_off + 2) as u32;
                let hi = u16_at(buf, unit_off + 4) as u32;
                let offset = (hi << 16 | lo) as i32;
                pc += 3;
                instructions.push(Instruction::FillArrayData {
                    array: aa,
                    payload_offset: offset,
                });
                continue;
            }

            0x27 => Instruction::Throw {
                exception: hi8(unit0),
            },

            0x28 => {
                let offset = hi8(unit0) as i8;
                Instruction::Goto { offset }
            }
            0x29 => {
                let offset = u16_at(buf, unit_off + 2) as i16;
                Instruction::Goto16 { offset }
            }
            0x2a => {
                let lo = u16_at(buf, unit_off + 2) as u32;
                let hi = u16_at(buf, unit_off + 4) as u32;
                let offset = (hi << 16 | lo) as i32;
                pc += 3;
                instructions.push(Instruction::Goto32 { offset });
                continue;
            }

            0x2b => {
                let aa = hi8(unit0);
                let lo = u16_at(buf, unit_off + 2) as u32;
                let hi = u16_at(buf, unit_off + 4) as u32;
                let offset = (hi << 16 | lo) as i32;
                pc += 3;
                instructions.push(Instruction::PackedSwitch {
                    test: aa,
                    payload_offset: offset,
                });
                continue;
            }
            0x2c => {
                let aa = hi8(unit0);
                let lo = u16_at(buf, unit_off + 2) as u32;
                let hi = u16_at(buf, unit_off + 4) as u32;
                let offset = (hi << 16 | lo) as i32;
                pc += 3;
                instructions.push(Instruction::SparseSwitch {
                    test: aa,
                    payload_offset: offset,
                });
                continue;
            }

            0x2d => {
                let (aa, bb, cc) = decode_23x(buf, unit_off);
                Instruction::CmpLFloat {
                    dest: aa,
                    a: bb,
                    b: cc,
                }
            }
            0x2e => {
                let (aa, bb, cc) = decode_23x(buf, unit_off);
                Instruction::CmpGFloat {
                    dest: aa,
                    a: bb,
                    b: cc,
                }
            }
            0x2f => {
                let (aa, bb, cc) = decode_23x(buf, unit_off);
                Instruction::CmpLDouble {
                    dest: aa,
                    a: bb,
                    b: cc,
                }
            }
            0x30 => {
                let (aa, bb, cc) = decode_23x(buf, unit_off);
                Instruction::CmpGDouble {
                    dest: aa,
                    a: bb,
                    b: cc,
                }
            }
            0x31 => {
                let (aa, bb, cc) = decode_23x(buf, unit_off);
                Instruction::CmpLong {
                    dest: aa,
                    a: bb,
                    b: cc,
                }
            }

            0x32 => {
                let (a, b) = nibbles(unit0);
                let off = u16_at(buf, unit_off + 2) as i16;
                Instruction::IfEq { a, b, offset: off }
            }
            0x33 => {
                let (a, b) = nibbles(unit0);
                let off = u16_at(buf, unit_off + 2) as i16;
                Instruction::IfNe { a, b, offset: off }
            }
            0x34 => {
                let (a, b) = nibbles(unit0);
                let off = u16_at(buf, unit_off + 2) as i16;
                Instruction::IfLt { a, b, offset: off }
            }
            0x35 => {
                let (a, b) = nibbles(unit0);
                let off = u16_at(buf, unit_off + 2) as i16;
                Instruction::IfGe { a, b, offset: off }
            }
            0x36 => {
                let (a, b) = nibbles(unit0);
                let off = u16_at(buf, unit_off + 2) as i16;
                Instruction::IfGt { a, b, offset: off }
            }
            0x37 => {
                let (a, b) = nibbles(unit0);
                let off = u16_at(buf, unit_off + 2) as i16;
                Instruction::IfLe { a, b, offset: off }
            }

            0x38 => {
                let aa = hi8(unit0);
                let off = u16_at(buf, unit_off + 2) as i16;
                Instruction::IfEqz { a: aa, offset: off }
            }
            0x39 => {
                let aa = hi8(unit0);
                let off = u16_at(buf, unit_off + 2) as i16;
                Instruction::IfNez { a: aa, offset: off }
            }
            0x3a => {
                let aa = hi8(unit0);
                let off = u16_at(buf, unit_off + 2) as i16;
                Instruction::IfLtz { a: aa, offset: off }
            }
            0x3b => {
                let aa = hi8(unit0);
                let off = u16_at(buf, unit_off + 2) as i16;
                Instruction::IfGez { a: aa, offset: off }
            }
            0x3c => {
                let aa = hi8(unit0);
                let off = u16_at(buf, unit_off + 2) as i16;
                Instruction::IfGtz { a: aa, offset: off }
            }
            0x3d => {
                let aa = hi8(unit0);
                let off = u16_at(buf, unit_off + 2) as i16;
                Instruction::IfLez { a: aa, offset: off }
            }

            0x3e..=0x43 => Instruction::Nop,

            0x44..=0x51 => {
                let (a, b, c) = decode_23x(buf, unit_off);
                match opcode {
                    0x44 => Instruction::Aget {
                        dest: a,
                        array: b,
                        index: c,
                    },
                    0x45 => Instruction::AgetWide {
                        dest: a,
                        array: b,
                        index: c,
                    },
                    0x46 => Instruction::AgetObject {
                        dest: a,
                        array: b,
                        index: c,
                    },
                    0x47 => Instruction::AgetBoolean {
                        dest: a,
                        array: b,
                        index: c,
                    },
                    0x48 => Instruction::AgetByte {
                        dest: a,
                        array: b,
                        index: c,
                    },
                    0x49 => Instruction::AgetChar {
                        dest: a,
                        array: b,
                        index: c,
                    },
                    0x4a => Instruction::AgetShort {
                        dest: a,
                        array: b,
                        index: c,
                    },
                    0x4b => Instruction::Aput {
                        src: a,
                        array: b,
                        index: c,
                    },
                    0x4c => Instruction::AputWide {
                        src: a,
                        array: b,
                        index: c,
                    },
                    0x4d => Instruction::AputObject {
                        src: a,
                        array: b,
                        index: c,
                    },
                    0x4e => Instruction::AputBoolean {
                        src: a,
                        array: b,
                        index: c,
                    },
                    0x4f => Instruction::AputByte {
                        src: a,
                        array: b,
                        index: c,
                    },
                    0x50 => Instruction::AputChar {
                        src: a,
                        array: b,
                        index: c,
                    },
                    0x51 => Instruction::AputShort {
                        src: a,
                        array: b,
                        index: c,
                    },
                    _ => unreachable!(),
                }
            }

            0x52..=0x5f => {
                let (a, b) = nibbles(unit0);
                let f = crate::types::FieldIdx(u16_at(buf, unit_off + 2) as u32);
                match opcode {
                    0x52 => Instruction::Iget {
                        dest: a,
                        obj: b,
                        field: f,
                    },
                    0x53 => Instruction::IgetWide {
                        dest: a,
                        obj: b,
                        field: f,
                    },
                    0x54 => Instruction::IgetObject {
                        dest: a,
                        obj: b,
                        field: f,
                    },
                    0x55 => Instruction::IgetBoolean {
                        dest: a,
                        obj: b,
                        field: f,
                    },
                    0x56 => Instruction::IgetByte {
                        dest: a,
                        obj: b,
                        field: f,
                    },
                    0x57 => Instruction::IgetChar {
                        dest: a,
                        obj: b,
                        field: f,
                    },
                    0x58 => Instruction::IgetShort {
                        dest: a,
                        obj: b,
                        field: f,
                    },
                    0x59 => Instruction::Iput {
                        src: a,
                        obj: b,
                        field: f,
                    },
                    0x5a => Instruction::IputWide {
                        src: a,
                        obj: b,
                        field: f,
                    },
                    0x5b => Instruction::IputObject {
                        src: a,
                        obj: b,
                        field: f,
                    },
                    0x5c => Instruction::IputBoolean {
                        src: a,
                        obj: b,
                        field: f,
                    },
                    0x5d => Instruction::IputByte {
                        src: a,
                        obj: b,
                        field: f,
                    },
                    0x5e => Instruction::IputChar {
                        src: a,
                        obj: b,
                        field: f,
                    },
                    0x5f => Instruction::IputShort {
                        src: a,
                        obj: b,
                        field: f,
                    },
                    _ => unreachable!(),
                }
            }

            0x60..=0x6d => {
                let aa = hi8(unit0);
                let f = crate::types::FieldIdx(u16_at(buf, unit_off + 2) as u32);
                match opcode {
                    0x60 => Instruction::Sget { dest: aa, field: f },
                    0x61 => Instruction::SgetWide { dest: aa, field: f },
                    0x62 => Instruction::SgetObject { dest: aa, field: f },
                    0x63 => Instruction::SgetBoolean { dest: aa, field: f },
                    0x64 => Instruction::SgetByte { dest: aa, field: f },
                    0x65 => Instruction::SgetChar { dest: aa, field: f },
                    0x66 => Instruction::SgetShort { dest: aa, field: f },
                    0x67 => Instruction::Sput { src: aa, field: f },
                    0x68 => Instruction::SputWide { src: aa, field: f },
                    0x69 => Instruction::SputObject { src: aa, field: f },
                    0x6a => Instruction::SputBoolean { src: aa, field: f },
                    0x6b => Instruction::SputByte { src: aa, field: f },
                    0x6c => Instruction::SputChar { src: aa, field: f },
                    0x6d => Instruction::SputShort { src: aa, field: f },
                    _ => unreachable!(),
                }
            }

            0x6e..=0x72 => {
                let insn = decode_35c_invoke(buf, unit_off, opcode);
                pc += 3;
                instructions.push(insn);
                continue;
            }

            0x73 => Instruction::Nop,

            0x74..=0x78 => {
                let insn = decode_3rc_invoke(buf, unit_off, opcode);
                pc += 3;
                instructions.push(insn);
                continue;
            }

            0x79..=0x7a => Instruction::Nop,

            // unary 12x
            0x7b..=0x8f => {
                let (a, b) = nibbles(unit0);
                match opcode {
                    0x7b => Instruction::NegInt { dest: a, src: b },
                    0x7c => Instruction::NotInt { dest: a, src: b },
                    0x7d => Instruction::NegLong { dest: a, src: b },
                    0x7e => Instruction::NotLong { dest: a, src: b },
                    0x7f => Instruction::NegFloat { dest: a, src: b },
                    0x80 => Instruction::NegDouble { dest: a, src: b },
                    0x81 => Instruction::IntToLong { dest: a, src: b },
                    0x82 => Instruction::IntToFloat { dest: a, src: b },
                    0x83 => Instruction::IntToDouble { dest: a, src: b },
                    0x84 => Instruction::LongToInt { dest: a, src: b },
                    0x85 => Instruction::LongToFloat { dest: a, src: b },
                    0x86 => Instruction::LongToDouble { dest: a, src: b },
                    0x87 => Instruction::FloatToInt { dest: a, src: b },
                    0x88 => Instruction::FloatToLong { dest: a, src: b },
                    0x89 => Instruction::FloatToDouble { dest: a, src: b },
                    0x8a => Instruction::DoubleToInt { dest: a, src: b },
                    0x8b => Instruction::DoubleToLong { dest: a, src: b },
                    0x8c => Instruction::DoubleToFloat { dest: a, src: b },
                    0x8d => Instruction::IntToByte { dest: a, src: b },
                    0x8e => Instruction::IntToChar { dest: a, src: b },
                    0x8f => Instruction::IntToShort { dest: a, src: b },
                    _ => unreachable!(),
                }
            }

            0x90..=0xaf => {
                let (a, b, c) = decode_23x(buf, unit_off);
                match opcode {
                    0x90 => Instruction::AddInt {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0x91 => Instruction::SubInt {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0x92 => Instruction::MulInt {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0x93 => Instruction::DivInt {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0x94 => Instruction::RemInt {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0x95 => Instruction::AndInt {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0x96 => Instruction::OrInt {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0x97 => Instruction::XorInt {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0x98 => Instruction::ShlInt {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0x99 => Instruction::ShrInt {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0x9a => Instruction::UshrInt {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0x9b => Instruction::AddLong {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0x9c => Instruction::SubLong {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0x9d => Instruction::MulLong {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0x9e => Instruction::DivLong {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0x9f => Instruction::RemLong {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0xa0 => Instruction::AndLong {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0xa1 => Instruction::OrLong {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0xa2 => Instruction::XorLong {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0xa3 => Instruction::ShlLong {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0xa4 => Instruction::ShrLong {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0xa5 => Instruction::UshrLong {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0xa6 => Instruction::AddFloat {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0xa7 => Instruction::SubFloat {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0xa8 => Instruction::MulFloat {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0xa9 => Instruction::DivFloat {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0xaa => Instruction::RemFloat {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0xab => Instruction::AddDouble {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0xac => Instruction::SubDouble {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0xad => Instruction::MulDouble {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0xae => Instruction::DivDouble {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    0xaf => Instruction::RemDouble {
                        dest: a,
                        a: b,
                        b: c,
                    },
                    _ => unreachable!(),
                }
            }

            0xb0..=0xcf => {
                let (a, b) = nibbles(unit0);
                match opcode {
                    0xb0 => Instruction::AddInt2Addr { dest_a: a, b },
                    0xb1 => Instruction::SubInt2Addr { dest_a: a, b },
                    0xb2 => Instruction::MulInt2Addr { dest_a: a, b },
                    0xb3 => Instruction::DivInt2Addr { dest_a: a, b },
                    0xb4 => Instruction::RemInt2Addr { dest_a: a, b },
                    0xb5 => Instruction::AndInt2Addr { dest_a: a, b },
                    0xb6 => Instruction::OrInt2Addr { dest_a: a, b },
                    0xb7 => Instruction::XorInt2Addr { dest_a: a, b },
                    0xb8 => Instruction::ShlInt2Addr { dest_a: a, b },
                    0xb9 => Instruction::ShrInt2Addr { dest_a: a, b },
                    0xba => Instruction::UshrInt2Addr { dest_a: a, b },
                    0xbb => Instruction::AddLong2Addr { dest_a: a, b },
                    0xbc => Instruction::SubLong2Addr { dest_a: a, b },
                    0xbd => Instruction::MulLong2Addr { dest_a: a, b },
                    0xbe => Instruction::DivLong2Addr { dest_a: a, b },
                    0xbf => Instruction::RemLong2Addr { dest_a: a, b },
                    0xc0 => Instruction::AndLong2Addr { dest_a: a, b },
                    0xc1 => Instruction::OrLong2Addr { dest_a: a, b },
                    0xc2 => Instruction::XorLong2Addr { dest_a: a, b },
                    0xc3 => Instruction::ShlLong2Addr { dest_a: a, b },
                    0xc4 => Instruction::ShrLong2Addr { dest_a: a, b },
                    0xc5 => Instruction::UshrLong2Addr { dest_a: a, b },
                    0xc6 => Instruction::AddFloat2Addr { dest_a: a, b },
                    0xc7 => Instruction::SubFloat2Addr { dest_a: a, b },
                    0xc8 => Instruction::MulFloat2Addr { dest_a: a, b },
                    0xc9 => Instruction::DivFloat2Addr { dest_a: a, b },
                    0xca => Instruction::RemFloat2Addr { dest_a: a, b },
                    0xcb => Instruction::AddDouble2Addr { dest_a: a, b },
                    0xcc => Instruction::SubDouble2Addr { dest_a: a, b },
                    0xcd => Instruction::MulDouble2Addr { dest_a: a, b },
                    0xce => Instruction::DivDouble2Addr { dest_a: a, b },
                    0xcf => Instruction::RemDouble2Addr { dest_a: a, b },
                    _ => unreachable!(),
                }
            }

            0xd0..=0xd7 => {
                let (a, b) = nibbles(unit0);
                let lit = u16_at(buf, unit_off + 2) as i16;
                match opcode {
                    0xd0 => Instruction::AddIntLit16 {
                        dest: a,
                        src: b,
                        literal: lit,
                    },
                    0xd1 => Instruction::RsubIntLit16 {
                        dest: a,
                        src: b,
                        literal: lit,
                    },
                    0xd2 => Instruction::MulIntLit16 {
                        dest: a,
                        src: b,
                        literal: lit,
                    },
                    0xd3 => Instruction::DivIntLit16 {
                        dest: a,
                        src: b,
                        literal: lit,
                    },
                    0xd4 => Instruction::RemIntLit16 {
                        dest: a,
                        src: b,
                        literal: lit,
                    },
                    0xd5 => Instruction::AndIntLit16 {
                        dest: a,
                        src: b,
                        literal: lit,
                    },
                    0xd6 => Instruction::OrIntLit16 {
                        dest: a,
                        src: b,
                        literal: lit,
                    },
                    0xd7 => Instruction::XorIntLit16 {
                        dest: a,
                        src: b,
                        literal: lit,
                    },
                    _ => unreachable!(),
                }
            }

            0xd8..=0xe2 => {
                let aa = hi8(unit0);
                let u1 = u16_at(buf, unit_off + 2);
                match opcode {
                    0xd8 => Instruction::AddIntLit8 {
                        dest: aa,
                        src: u1 as u8,
                        literal: (u1 >> 8) as i8,
                    },
                    0xd9 => Instruction::RsubIntLit8 {
                        dest: aa,
                        src: u1 as u8,
                        literal: (u1 >> 8) as i8,
                    },
                    0xda => Instruction::MulIntLit8 {
                        dest: aa,
                        src: u1 as u8,
                        literal: (u1 >> 8) as i8,
                    },
                    0xdb => Instruction::DivIntLit8 {
                        dest: aa,
                        src: u1 as u8,
                        literal: (u1 >> 8) as i8,
                    },
                    0xdc => Instruction::RemIntLit8 {
                        dest: aa,
                        src: u1 as u8,
                        literal: (u1 >> 8) as i8,
                    },
                    0xdd => Instruction::AndIntLit8 {
                        dest: aa,
                        src: u1 as u8,
                        literal: (u1 >> 8) as i8,
                    },
                    0xde => Instruction::OrIntLit8 {
                        dest: aa,
                        src: u1 as u8,
                        literal: (u1 >> 8) as i8,
                    },
                    0xdf => Instruction::XorIntLit8 {
                        dest: aa,
                        src: u1 as u8,
                        literal: (u1 >> 8) as i8,
                    },
                    0xe0 => Instruction::ShlIntLit8 {
                        dest: aa,
                        src: u1 as u8,
                        literal: (u1 >> 8) as i8,
                    },
                    0xe1 => Instruction::ShrIntLit8 {
                        dest: aa,
                        src: u1 as u8,
                        literal: (u1 >> 8) as i8,
                    },
                    0xe2 => Instruction::UshrIntLit8 {
                        dest: aa,
                        src: u1 as u8,
                        literal: (u1 >> 8) as i8,
                    },
                    _ => unreachable!(),
                }
            }

            0xe3..=0xf9 => Instruction::Nop,

            0xfa..=0xfd => {
                let insn = decode_invoke_polymorphic(buf, unit_off, opcode);
                pc += match opcode {
                    0xfa | 0xfb => 4,
                    _ => 3,
                };
                instructions.push(insn);
                continue;
            }
        };

        let units = match opcode {
            0x00
            | 0x01
            | 0x04
            | 0x07
            | 0x0a..=0x12
            | 0x1d..=0x1e
            | 0x21
            | 0x27..=0x28
            | 0x3e..=0x43
            | 0x73
            | 0x79..=0x7a
            | 0x7b..=0x8f
            | 0xb0..=0xcf
            | 0xe3..=0xf9 => 1,
            _ => 2,
        };
        pc += units;
        instructions.push(insn);
    }

    Ok(instructions)
}
