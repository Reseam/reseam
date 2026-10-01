// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::{Result, malformed, require_len};
use crate::read::{i32_at, u16_at, u32_at};
use crate::types::instruction::Instruction;
use crate::types::instruction_encoding::opcodes::{
    ARRAY_LENGTH, CHECK_CAST, CMP_G_DOUBLE, CMP_G_FLOAT, CMP_L_DOUBLE, CMP_L_FLOAT, CMP_LONG,
    CONST, CONST_CLASS, CONST_HIGH16, CONST_METHOD_HANDLE, CONST_METHOD_TYPE, CONST_STRING,
    CONST_STRING_JUMBO, CONST_WIDE, CONST_WIDE_HIGH16, CONST_WIDE16, CONST_WIDE32, CONST4, CONST16,
    FILL_ARRAY_DATA, FILL_ARRAY_DATA_PAYLOAD, FILLED_NEW_ARRAY, FILLED_NEW_ARRAY_RANGE, GOTO,
    GOTO16, GOTO32, IF_EQ, IF_EQZ, IF_GE, IF_GEZ, IF_GT, IF_GTZ, IF_LE, IF_LEZ, IF_LT, IF_LTZ,
    IF_NE, IF_NEZ, INSTANCE_OF, MONITOR_ENTER, MONITOR_EXIT, MOVE, MOVE_EXCEPTION, MOVE_FROM16,
    MOVE_OBJECT, MOVE_OBJECT_FROM16, MOVE_OBJECT16, MOVE_RESULT, MOVE_RESULT_OBJECT,
    MOVE_RESULT_WIDE, MOVE_WIDE, MOVE_WIDE_FROM16, MOVE_WIDE16, MOVE16, NEW_ARRAY, NEW_INSTANCE,
    NOP, PACKED_SWITCH, PACKED_SWITCH_PAYLOAD, RETURN, RETURN_OBJECT, RETURN_VOID, RETURN_WIDE,
    SPARSE_SWITCH, SPARSE_SWITCH_PAYLOAD, THROW,
};

use super::super::arithmetic::decode_23x;
use super::super::memory::decode_35c_type;
use super::{hi8, nibbles};

#[expect(
    clippy::too_many_lines,
    reason = "the exhaustive format dispatch keeps each encoding visible in one match"
)]
pub(super) fn decode_opcode(buf: &[u8], unit_off: usize, opcode: u8) -> Result<Instruction> {
    let unit0 = u16_at(buf, unit_off);

    let decoded = match u16::from(opcode) {
        NOP => decode_nop_or_payload(buf, unit_off, unit0)?,

        MOVE => {
            let (a, b) = nibbles(unit0);
            Instruction::Move { dest: a, src: b }
        }
        MOVE_WIDE => {
            let (a, b) = nibbles(unit0);
            Instruction::MoveWide { dest: a, src: b }
        }
        MOVE_OBJECT => {
            let (a, b) = nibbles(unit0);
            Instruction::MoveObject { dest: a, src: b }
        }
        ARRAY_LENGTH => {
            let (a, b) = nibbles(unit0);
            Instruction::ArrayLength { dest: a, array: b }
        }

        MOVE_FROM16 => Instruction::MoveFrom16 {
            dest: hi8(unit0),
            src: u16_at(buf, unit_off + 2),
        },
        MOVE_WIDE_FROM16 => Instruction::MoveWideFrom16 {
            dest: hi8(unit0),
            src: u16_at(buf, unit_off + 2),
        },
        MOVE_OBJECT_FROM16 => Instruction::MoveObjectFrom16 {
            dest: hi8(unit0),
            src: u16_at(buf, unit_off + 2),
        },

        MOVE16 => Instruction::Move16 {
            dest: u16_at(buf, unit_off + 2),
            src: u16_at(buf, unit_off + 4),
        },
        MOVE_WIDE16 => Instruction::MoveWide16 {
            dest: u16_at(buf, unit_off + 2),
            src: u16_at(buf, unit_off + 4),
        },
        MOVE_OBJECT16 => Instruction::MoveObject16 {
            dest: u16_at(buf, unit_off + 2),
            src: u16_at(buf, unit_off + 4),
        },

        MOVE_RESULT => Instruction::MoveResult { dest: hi8(unit0) },
        MOVE_RESULT_WIDE => Instruction::MoveResultWide { dest: hi8(unit0) },
        MOVE_RESULT_OBJECT => Instruction::MoveResultObject { dest: hi8(unit0) },
        MOVE_EXCEPTION => Instruction::MoveException { dest: hi8(unit0) },

        RETURN_VOID => Instruction::ReturnVoid,
        RETURN => Instruction::Return { src: hi8(unit0) },
        RETURN_WIDE => Instruction::ReturnWide { src: hi8(unit0) },
        RETURN_OBJECT => Instruction::ReturnObject { src: hi8(unit0) },

        CONST4 => {
            let (a, b) = nibbles(unit0);
            let value = ((b as i8) << 4) >> 4;
            Instruction::Const4 { dest: a, value }
        }

        CONST16 => Instruction::Const16 {
            dest: hi8(unit0),
            value: u16_at(buf, unit_off + 2) as i16,
        },
        CONST_WIDE16 => Instruction::ConstWide16 {
            dest: hi8(unit0),
            value: u16_at(buf, unit_off + 2) as i16,
        },

        CONST => {
            let lo = u32::from(u16_at(buf, unit_off + 2));
            let hi = u32::from(u16_at(buf, unit_off + 4));
            Instruction::Const {
                dest: hi8(unit0),
                value: (hi << 16 | lo) as i32,
            }
        }
        CONST_WIDE32 => {
            let lo = u32::from(u16_at(buf, unit_off + 2));
            let hi = u32::from(u16_at(buf, unit_off + 4));
            Instruction::ConstWide32 {
                dest: hi8(unit0),
                value: (hi << 16 | lo) as i32,
            }
        }

        CONST_HIGH16 => Instruction::ConstHigh16 {
            dest: hi8(unit0),
            value: u16_at(buf, unit_off + 2) as i16,
        },
        CONST_WIDE_HIGH16 => Instruction::ConstWideHigh16 {
            dest: hi8(unit0),
            value: u16_at(buf, unit_off + 2) as i16,
        },

        CONST_WIDE => {
            let mut value: i64 = 0;
            for i in 0..4u64 {
                value |= i64::from(u16_at(buf, unit_off + 2 + i as usize * 2)) << (i * 16);
            }
            Instruction::ConstWide {
                dest: hi8(unit0),
                value,
            }
        }

        CONST_STRING => Instruction::ConstString {
            dest: hi8(unit0),
            string: crate::types::StringIdx(u32::from(u16_at(buf, unit_off + 2))),
        },
        CONST_CLASS => Instruction::ConstClass {
            dest: hi8(unit0),
            type_: crate::types::TypeIdx(u32::from(u16_at(buf, unit_off + 2))),
        },
        CONST_METHOD_HANDLE => Instruction::ConstMethodHandle {
            dest: hi8(unit0),
            method_handle: crate::types::method_handle::MethodHandleIdx(u32::from(u16_at(
                buf,
                unit_off + 2,
            ))),
        },
        CONST_METHOD_TYPE => Instruction::ConstMethodType {
            dest: hi8(unit0),
            proto: crate::types::ProtoIdx(u32::from(u16_at(buf, unit_off + 2))),
        },

        CONST_STRING_JUMBO => {
            let lo = u32::from(u16_at(buf, unit_off + 2));
            let hi = u32::from(u16_at(buf, unit_off + 4));
            Instruction::ConstStringJumbo {
                dest: hi8(unit0),
                string: crate::types::StringIdx(hi << 16 | lo),
            }
        }

        MONITOR_ENTER => Instruction::MonitorEnter { ref_: hi8(unit0) },
        MONITOR_EXIT => Instruction::MonitorExit { ref_: hi8(unit0) },

        CHECK_CAST => Instruction::CheckCast {
            ref_: hi8(unit0),
            type_: crate::types::TypeIdx(u32::from(u16_at(buf, unit_off + 2))),
        },

        INSTANCE_OF => {
            let (a, b) = nibbles(unit0);
            Instruction::InstanceOf {
                dest: a,
                ref_: b,
                type_: crate::types::TypeIdx(u32::from(u16_at(buf, unit_off + 2))),
            }
        }

        NEW_INSTANCE => Instruction::NewInstance {
            dest: hi8(unit0),
            type_: crate::types::TypeIdx(u32::from(u16_at(buf, unit_off + 2))),
        },

        NEW_ARRAY => {
            let (a, b) = nibbles(unit0);
            Instruction::NewArray {
                dest: a,
                size: b,
                type_: crate::types::TypeIdx(u32::from(u16_at(buf, unit_off + 2))),
            }
        }

        FILLED_NEW_ARRAY => decode_35c_type(buf, unit_off)?,
        FILLED_NEW_ARRAY_RANGE => Instruction::FilledNewArrayRange {
            type_: crate::types::TypeIdx(u32::from(u16_at(buf, unit_off + 2))),
            first_reg: u16_at(buf, unit_off + 4),
            count: hi8(unit0),
        },
        FILL_ARRAY_DATA => {
            let lo = u32::from(u16_at(buf, unit_off + 2));
            let hi = u32::from(u16_at(buf, unit_off + 4));
            Instruction::FillArrayData {
                array: hi8(unit0),
                payload_offset: (hi << 16 | lo) as i32,
            }
        }

        THROW => Instruction::Throw {
            exception: hi8(unit0),
        },
        GOTO => Instruction::Goto {
            offset: hi8(unit0) as i8,
        },
        GOTO16 => Instruction::Goto16 {
            offset: u16_at(buf, unit_off + 2) as i16,
        },
        GOTO32 => {
            let lo = u32::from(u16_at(buf, unit_off + 2));
            let hi = u32::from(u16_at(buf, unit_off + 4));
            Instruction::Goto32 {
                offset: (hi << 16 | lo) as i32,
            }
        }

        PACKED_SWITCH => {
            let lo = u32::from(u16_at(buf, unit_off + 2));
            let hi = u32::from(u16_at(buf, unit_off + 4));
            Instruction::PackedSwitch {
                test: hi8(unit0),
                payload_offset: (hi << 16 | lo) as i32,
            }
        }
        SPARSE_SWITCH => {
            let lo = u32::from(u16_at(buf, unit_off + 2));
            let hi = u32::from(u16_at(buf, unit_off + 4));
            Instruction::SparseSwitch {
                test: hi8(unit0),
                payload_offset: (hi << 16 | lo) as i32,
            }
        }

        CMP_L_FLOAT | CMP_G_FLOAT | CMP_L_DOUBLE | CMP_G_DOUBLE | CMP_LONG => {
            decode_cmp(buf, unit_off, opcode)?
        }

        IF_EQ..=IF_LE => decode_if_test(unit0, buf, unit_off, opcode)?,
        IF_EQZ..=IF_LEZ => decode_if_testz(unit0, buf, unit_off, opcode)?,

        _ => {
            return Err(malformed(
                "instruction opcode",
                unit_off,
                "opcode is outside the decoded family",
            ));
        }
    };

    Ok(decoded)
}

fn decode_nop_or_payload(buf: &[u8], unit_off: usize, unit0: u16) -> Result<Instruction> {
    let decoded = match unit0 {
        PACKED_SWITCH_PAYLOAD => {
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
            Instruction::PackedSwitchPayload(Box::new(
                crate::types::instruction::PackedSwitchData { first_key, targets },
            ))
        }
        SPARSE_SWITCH_PAYLOAD => {
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
            Instruction::SparseSwitchPayload(Box::new(
                crate::types::instruction::SparseSwitchData { keys_and_targets },
            ))
        }
        FILL_ARRAY_DATA_PAYLOAD => {
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
            Instruction::FillArrayDataPayload(Box::new(
                crate::types::instruction::FillArrayPayloadData {
                    element_width,
                    data,
                },
            ))
        }
        _ => Instruction::Nop,
    };

    Ok(decoded)
}

fn decode_cmp(buf: &[u8], unit_off: usize, opcode: u8) -> Result<Instruction> {
    let [dest, a, b] = decode_23x(buf, unit_off);

    Ok(match u16::from(opcode) {
        CMP_L_FLOAT => Instruction::CmpLFloat { dest, a, b },
        CMP_G_FLOAT => Instruction::CmpGFloat { dest, a, b },
        CMP_L_DOUBLE => Instruction::CmpLDouble { dest, a, b },
        CMP_G_DOUBLE => Instruction::CmpGDouble { dest, a, b },
        CMP_LONG => Instruction::CmpLong { dest, a, b },
        _ => {
            return Err(malformed(
                "instruction opcode",
                unit_off,
                "opcode is outside the decoded family",
            ));
        }
    })
}

fn decode_if_test(unit0: u16, buf: &[u8], unit_off: usize, opcode: u8) -> Result<Instruction> {
    let (a, b) = nibbles(unit0);
    let offset = u16_at(buf, unit_off + 2) as i16;

    Ok(match u16::from(opcode) {
        IF_EQ => Instruction::IfEq { a, b, offset },
        IF_NE => Instruction::IfNe { a, b, offset },
        IF_LT => Instruction::IfLt { a, b, offset },
        IF_GE => Instruction::IfGe { a, b, offset },
        IF_GT => Instruction::IfGt { a, b, offset },
        IF_LE => Instruction::IfLe { a, b, offset },
        _ => {
            return Err(malformed(
                "instruction opcode",
                unit_off,
                "opcode is outside the decoded family",
            ));
        }
    })
}

fn decode_if_testz(unit0: u16, buf: &[u8], unit_off: usize, opcode: u8) -> Result<Instruction> {
    let a = hi8(unit0);
    let offset = u16_at(buf, unit_off + 2) as i16;

    Ok(match u16::from(opcode) {
        IF_EQZ => Instruction::IfEqz { a, offset },
        IF_NEZ => Instruction::IfNez { a, offset },
        IF_LTZ => Instruction::IfLtz { a, offset },
        IF_GEZ => Instruction::IfGez { a, offset },
        IF_GTZ => Instruction::IfGtz { a, offset },
        IF_LEZ => Instruction::IfLez { a, offset },
        _ => {
            return Err(malformed(
                "instruction opcode",
                unit_off,
                "opcode is outside the decoded family",
            ));
        }
    })
}
