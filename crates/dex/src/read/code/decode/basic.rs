// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::{malformed, require_len, Result};
use crate::types::instruction::Instruction;

use super::super::arithmetic::decode_23x;
use super::super::format::{i32_at, u16_at, u32_at};
use super::super::memory::decode_35c_type;
use super::{hi8, nibbles, DecodedInstruction};

pub(super) fn decode_opcode(buf: &[u8], unit_off: usize, opcode: u8) -> Result<DecodedInstruction> {
    let unit0 = u16_at(buf, unit_off);

    let decoded = match opcode {
        0x00 => decode_nop_or_payload(buf, unit_off, unit0)?,

        0x01 => {
            let (a, b) = nibbles(unit0);
            DecodedInstruction::new(Instruction::Move { dest: a, src: b }, 1)
        }
        0x04 => {
            let (a, b) = nibbles(unit0);
            DecodedInstruction::new(Instruction::MoveWide { dest: a, src: b }, 1)
        }
        0x07 => {
            let (a, b) = nibbles(unit0);
            DecodedInstruction::new(Instruction::MoveObject { dest: a, src: b }, 1)
        }
        0x21 => {
            let (a, b) = nibbles(unit0);
            DecodedInstruction::new(Instruction::ArrayLength { dest: a, array: b }, 1)
        }

        0x02 => DecodedInstruction::new(
            Instruction::MoveFrom16 {
                dest: hi8(unit0),
                src: u16_at(buf, unit_off + 2),
            },
            2,
        ),
        0x05 => DecodedInstruction::new(
            Instruction::MoveWideFrom16 {
                dest: hi8(unit0),
                src: u16_at(buf, unit_off + 2),
            },
            2,
        ),
        0x08 => DecodedInstruction::new(
            Instruction::MoveObjectFrom16 {
                dest: hi8(unit0),
                src: u16_at(buf, unit_off + 2),
            },
            2,
        ),

        0x03 => DecodedInstruction::new(
            Instruction::Move16 {
                dest: u16_at(buf, unit_off + 2),
                src: u16_at(buf, unit_off + 4),
            },
            3,
        ),
        0x06 => DecodedInstruction::new(
            Instruction::MoveWide16 {
                dest: u16_at(buf, unit_off + 2),
                src: u16_at(buf, unit_off + 4),
            },
            3,
        ),
        0x09 => DecodedInstruction::new(
            Instruction::MoveObject16 {
                dest: u16_at(buf, unit_off + 2),
                src: u16_at(buf, unit_off + 4),
            },
            3,
        ),

        0x0a => DecodedInstruction::new(Instruction::MoveResult { dest: hi8(unit0) }, 1),
        0x0b => DecodedInstruction::new(Instruction::MoveResultWide { dest: hi8(unit0) }, 1),
        0x0c => DecodedInstruction::new(Instruction::MoveResultObject { dest: hi8(unit0) }, 1),
        0x0d => DecodedInstruction::new(Instruction::MoveException { dest: hi8(unit0) }, 1),

        0x0e => DecodedInstruction::new(Instruction::ReturnVoid, 1),
        0x0f => DecodedInstruction::new(Instruction::Return { src: hi8(unit0) }, 1),
        0x10 => DecodedInstruction::new(Instruction::ReturnWide { src: hi8(unit0) }, 1),
        0x11 => DecodedInstruction::new(Instruction::ReturnObject { src: hi8(unit0) }, 1),

        0x12 => {
            let (a, b) = nibbles(unit0);
            let value = ((b as i8) << 4) >> 4;
            DecodedInstruction::new(Instruction::Const4 { dest: a, value }, 1)
        }

        0x13 => DecodedInstruction::new(
            Instruction::Const16 {
                dest: hi8(unit0),
                value: u16_at(buf, unit_off + 2) as i16,
            },
            2,
        ),
        0x16 => DecodedInstruction::new(
            Instruction::ConstWide16 {
                dest: hi8(unit0),
                value: u16_at(buf, unit_off + 2) as i16,
            },
            2,
        ),

        0x14 => {
            let lo = u16_at(buf, unit_off + 2) as u32;
            let hi = u16_at(buf, unit_off + 4) as u32;
            DecodedInstruction::new(
                Instruction::Const {
                    dest: hi8(unit0),
                    value: (hi << 16 | lo) as i32,
                },
                3,
            )
        }
        0x17 => {
            let lo = u16_at(buf, unit_off + 2) as u32;
            let hi = u16_at(buf, unit_off + 4) as u32;
            DecodedInstruction::new(
                Instruction::ConstWide32 {
                    dest: hi8(unit0),
                    value: (hi << 16 | lo) as i32,
                },
                3,
            )
        }

        0x15 => DecodedInstruction::new(
            Instruction::ConstHigh16 {
                dest: hi8(unit0),
                value: u16_at(buf, unit_off + 2) as i16,
            },
            2,
        ),
        0x19 => DecodedInstruction::new(
            Instruction::ConstWideHigh16 {
                dest: hi8(unit0),
                value: u16_at(buf, unit_off + 2) as i16,
            },
            2,
        ),

        0x18 => {
            let mut value: i64 = 0;
            for i in 0..4u64 {
                value |= (u16_at(buf, unit_off + 2 + i as usize * 2) as i64) << (i * 16);
            }
            DecodedInstruction::new(
                Instruction::ConstWide {
                    dest: hi8(unit0),
                    value,
                },
                5,
            )
        }

        0x1a => DecodedInstruction::new(
            Instruction::ConstString {
                dest: hi8(unit0),
                string: crate::types::StringIdx(u16_at(buf, unit_off + 2) as u32),
            },
            2,
        ),
        0x1c => DecodedInstruction::new(
            Instruction::ConstClass {
                dest: hi8(unit0),
                type_: crate::types::TypeIdx(u16_at(buf, unit_off + 2) as u32),
            },
            2,
        ),
        0xfe => DecodedInstruction::new(
            Instruction::ConstMethodHandle {
                dest: hi8(unit0),
                method_handle: crate::types::method_handle::MethodHandleIdx(u16_at(
                    buf,
                    unit_off + 2,
                )
                    as u32),
            },
            2,
        ),
        0xff => DecodedInstruction::new(
            Instruction::ConstMethodType {
                dest: hi8(unit0),
                proto: crate::types::ProtoIdx(u16_at(buf, unit_off + 2)),
            },
            2,
        ),

        0x1b => {
            let lo = u16_at(buf, unit_off + 2) as u32;
            let hi = u16_at(buf, unit_off + 4) as u32;
            DecodedInstruction::new(
                Instruction::ConstStringJumbo {
                    dest: hi8(unit0),
                    string: crate::types::StringIdx(hi << 16 | lo),
                },
                3,
            )
        }

        0x1d => DecodedInstruction::new(Instruction::MonitorEnter { ref_: hi8(unit0) }, 1),
        0x1e => DecodedInstruction::new(Instruction::MonitorExit { ref_: hi8(unit0) }, 1),

        0x1f => DecodedInstruction::new(
            Instruction::CheckCast {
                ref_: hi8(unit0),
                type_: crate::types::TypeIdx(u16_at(buf, unit_off + 2) as u32),
            },
            2,
        ),

        0x20 => {
            let (a, b) = nibbles(unit0);
            DecodedInstruction::new(
                Instruction::InstanceOf {
                    dest: a,
                    ref_: b,
                    type_: crate::types::TypeIdx(u16_at(buf, unit_off + 2) as u32),
                },
                2,
            )
        }

        0x22 => DecodedInstruction::new(
            Instruction::NewInstance {
                dest: hi8(unit0),
                type_: crate::types::TypeIdx(u16_at(buf, unit_off + 2) as u32),
            },
            2,
        ),

        0x23 => {
            let (a, b) = nibbles(unit0);
            DecodedInstruction::new(
                Instruction::NewArray {
                    dest: a,
                    size: b,
                    type_: crate::types::TypeIdx(u16_at(buf, unit_off + 2) as u32),
                },
                2,
            )
        }

        0x24 => DecodedInstruction::new(decode_35c_type(buf, unit_off), 3),
        0x25 => DecodedInstruction::new(
            Instruction::FilledNewArrayRange {
                type_: crate::types::TypeIdx(u16_at(buf, unit_off + 2) as u32),
                first_reg: u16_at(buf, unit_off + 4),
                count: hi8(unit0),
            },
            3,
        ),
        0x26 => {
            let lo = u16_at(buf, unit_off + 2) as u32;
            let hi = u16_at(buf, unit_off + 4) as u32;
            DecodedInstruction::new(
                Instruction::FillArrayData {
                    array: hi8(unit0),
                    payload_offset: (hi << 16 | lo) as i32,
                },
                3,
            )
        }

        0x27 => DecodedInstruction::new(
            Instruction::Throw {
                exception: hi8(unit0),
            },
            1,
        ),
        0x28 => DecodedInstruction::new(
            Instruction::Goto {
                offset: hi8(unit0) as i8,
            },
            1,
        ),
        0x29 => DecodedInstruction::new(
            Instruction::Goto16 {
                offset: u16_at(buf, unit_off + 2) as i16,
            },
            2,
        ),
        0x2a => {
            let lo = u16_at(buf, unit_off + 2) as u32;
            let hi = u16_at(buf, unit_off + 4) as u32;
            DecodedInstruction::new(
                Instruction::Goto32 {
                    offset: (hi << 16 | lo) as i32,
                },
                3,
            )
        }

        0x2b => {
            let lo = u16_at(buf, unit_off + 2) as u32;
            let hi = u16_at(buf, unit_off + 4) as u32;
            DecodedInstruction::new(
                Instruction::PackedSwitch {
                    test: hi8(unit0),
                    payload_offset: (hi << 16 | lo) as i32,
                },
                3,
            )
        }
        0x2c => {
            let lo = u16_at(buf, unit_off + 2) as u32;
            let hi = u16_at(buf, unit_off + 4) as u32;
            DecodedInstruction::new(
                Instruction::SparseSwitch {
                    test: hi8(unit0),
                    payload_offset: (hi << 16 | lo) as i32,
                },
                3,
            )
        }

        0x2d => decode_cmp(buf, unit_off, opcode),
        0x2e => decode_cmp(buf, unit_off, opcode),
        0x2f => decode_cmp(buf, unit_off, opcode),
        0x30 => decode_cmp(buf, unit_off, opcode),
        0x31 => decode_cmp(buf, unit_off, opcode),

        0x32..=0x37 => decode_if_test(unit0, buf, unit_off, opcode),
        0x38..=0x3d => decode_if_testz(unit0, buf, unit_off, opcode),
        0x3e..=0x43 => DecodedInstruction::new(Instruction::Nop, 1),

        _ => unreachable!(),
    };

    Ok(decoded)
}

fn decode_nop_or_payload(buf: &[u8], unit_off: usize, unit0: u16) -> Result<DecodedInstruction> {
    let decoded = match unit0 {
        0x0100 => {
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
            DecodedInstruction::new(
                Instruction::PackedSwitchPayload(Box::new(
                    crate::types::instruction::PackedSwitchData { first_key, targets },
                )),
                1 + 1 + 2 + size * 2,
            )
        }
        0x0200 => {
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
            DecodedInstruction::new(
                Instruction::SparseSwitchPayload(Box::new(
                    crate::types::instruction::SparseSwitchData { keys_and_targets },
                )),
                1 + 1 + size * 2 + size * 2,
            )
        }
        0x0300 => {
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
            DecodedInstruction::new(
                Instruction::FillArrayDataPayload(Box::new(
                    crate::types::instruction::FillArrayPayloadData {
                        element_width,
                        data,
                    },
                )),
                (8 + data_bytes).div_ceil(2),
            )
        }
        _ => DecodedInstruction::new(Instruction::Nop, 1),
    };

    Ok(decoded)
}

fn decode_cmp(buf: &[u8], unit_off: usize, opcode: u8) -> DecodedInstruction {
    let (dest, a, b) = decode_23x(buf, unit_off);
    let instruction = match opcode {
        0x2d => Instruction::CmpLFloat { dest, a, b },
        0x2e => Instruction::CmpGFloat { dest, a, b },
        0x2f => Instruction::CmpLDouble { dest, a, b },
        0x30 => Instruction::CmpGDouble { dest, a, b },
        0x31 => Instruction::CmpLong { dest, a, b },
        _ => unreachable!(),
    };
    DecodedInstruction::new(instruction, 2)
}

fn decode_if_test(unit0: u16, buf: &[u8], unit_off: usize, opcode: u8) -> DecodedInstruction {
    let (a, b) = nibbles(unit0);
    let offset = u16_at(buf, unit_off + 2) as i16;
    let instruction = match opcode {
        0x32 => Instruction::IfEq { a, b, offset },
        0x33 => Instruction::IfNe { a, b, offset },
        0x34 => Instruction::IfLt { a, b, offset },
        0x35 => Instruction::IfGe { a, b, offset },
        0x36 => Instruction::IfGt { a, b, offset },
        0x37 => Instruction::IfLe { a, b, offset },
        _ => unreachable!(),
    };
    DecodedInstruction::new(instruction, 2)
}

fn decode_if_testz(unit0: u16, buf: &[u8], unit_off: usize, opcode: u8) -> DecodedInstruction {
    let a = hi8(unit0);
    let offset = u16_at(buf, unit_off + 2) as i16;
    let instruction = match opcode {
        0x38 => Instruction::IfEqz { a, offset },
        0x39 => Instruction::IfNez { a, offset },
        0x3a => Instruction::IfLtz { a, offset },
        0x3b => Instruction::IfGez { a, offset },
        0x3c => Instruction::IfGtz { a, offset },
        0x3d => Instruction::IfLez { a, offset },
        _ => unreachable!(),
    };
    DecodedInstruction::new(instruction, 2)
}
