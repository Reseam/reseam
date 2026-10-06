// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::error::{Result, malformed};
use crate::read::u16_at;
use crate::types::instruction::{Instruction, RegList};
use crate::types::instruction_encoding::opcodes::{
    INVOKE_CUSTOM, INVOKE_CUSTOM_RANGE, INVOKE_DIRECT, INVOKE_DIRECT_RANGE, INVOKE_INTERFACE,
    INVOKE_INTERFACE_RANGE, INVOKE_POLYMORPHIC, INVOKE_POLYMORPHIC_RANGE, INVOKE_STATIC,
    INVOKE_STATIC_RANGE, INVOKE_SUPER, INVOKE_SUPER_RANGE, INVOKE_VIRTUAL, INVOKE_VIRTUAL_RANGE,
};
use crate::types::method_handle::CallSiteIdx;
use crate::types::{MethodIdx, ProtoIdx};

fn hi8(unit: u16) -> u8 {
    (unit >> 8) as u8
}

pub(crate) fn decode_35c_args(
    count: u8,
    reg_unit: u16,
    unit0: u16,
    offset: usize,
) -> Result<RegList> {
    if count > 5 {
        return Err(malformed(
            "compact instruction",
            offset,
            "argument count exceeds five registers",
        ));
    }
    let regs = [
        (reg_unit & 0xF) as u8,
        ((reg_unit >> 4) & 0xF) as u8,
        ((reg_unit >> 8) & 0xF) as u8,
        ((reg_unit >> 12) & 0xF) as u8,
        ((unit0 >> 8) & 0xF) as u8,
    ];

    RegList::try_from_iter(regs.into_iter().take(count as usize))
}

pub fn decode_35c_invoke(buf: &[u8], off: usize, opcode: u8) -> Result<Instruction> {
    let unit0 = u16_at(buf, off);
    let count = ((unit0 >> 12) & 0xF) as u8;
    let method = MethodIdx(u32::from(u16_at(buf, off + 2)));
    let reg_unit = u16_at(buf, off + 4);
    let args = decode_35c_args(count, reg_unit, unit0, off)?;

    Ok(match u16::from(opcode) {
        INVOKE_VIRTUAL => Instruction::InvokeVirtual { method, args },
        INVOKE_SUPER => Instruction::InvokeSuper { method, args },
        INVOKE_DIRECT => Instruction::InvokeDirect { method, args },
        INVOKE_STATIC => Instruction::InvokeStatic { method, args },
        INVOKE_INTERFACE => Instruction::InvokeInterface { method, args },
        _ => {
            return Err(malformed(
                "invoke opcode",
                off,
                "opcode is outside the decoded family",
            ));
        }
    })
}

pub fn decode_3rc_invoke(buf: &[u8], off: usize, opcode: u8) -> Result<Instruction> {
    let unit0 = u16_at(buf, off);
    let count = hi8(unit0);
    let method = MethodIdx(u32::from(u16_at(buf, off + 2)));
    let first_reg = u16_at(buf, off + 4);

    Ok(match u16::from(opcode) {
        INVOKE_VIRTUAL_RANGE => Instruction::InvokeVirtualRange {
            method,
            first_reg,
            count,
        },
        INVOKE_SUPER_RANGE => Instruction::InvokeSuperRange {
            method,
            first_reg,
            count,
        },
        INVOKE_DIRECT_RANGE => Instruction::InvokeDirectRange {
            method,
            first_reg,
            count,
        },
        INVOKE_STATIC_RANGE => Instruction::InvokeStaticRange {
            method,
            first_reg,
            count,
        },
        INVOKE_INTERFACE_RANGE => Instruction::InvokeInterfaceRange {
            method,
            first_reg,
            count,
        },
        _ => {
            return Err(malformed(
                "invoke opcode",
                off,
                "opcode is outside the decoded family",
            ));
        }
    })
}

pub fn decode_invoke_polymorphic(buf: &[u8], off: usize, opcode: u8) -> Result<Instruction> {
    let unit0 = u16_at(buf, off);
    Ok(match u16::from(opcode) {
        INVOKE_POLYMORPHIC => {
            let count = ((unit0 >> 12) & 0xF) as u8;
            let method = MethodIdx(u32::from(u16_at(buf, off + 2)));
            let reg_unit = u16_at(buf, off + 4);
            let proto = ProtoIdx(u32::from(u16_at(buf, off + 6)));
            let args = decode_35c_args(count, reg_unit, unit0, off)?;
            Instruction::InvokePolymorphic {
                method,
                proto,
                args,
            }
        }
        INVOKE_POLYMORPHIC_RANGE => {
            let count = hi8(unit0);
            let method = MethodIdx(u32::from(u16_at(buf, off + 2)));
            let first_reg = u16_at(buf, off + 4);
            let proto = ProtoIdx(u32::from(u16_at(buf, off + 6)));
            Instruction::InvokePolymorphicRange {
                method,
                proto,
                first_reg,
                count,
            }
        }
        INVOKE_CUSTOM => {
            let count = ((unit0 >> 12) & 0xF) as u8;
            let call_site = CallSiteIdx(u32::from(u16_at(buf, off + 2)));
            let reg_unit = u16_at(buf, off + 4);
            let args = decode_35c_args(count, reg_unit, unit0, off)?;
            Instruction::InvokeCustom { call_site, args }
        }
        INVOKE_CUSTOM_RANGE => {
            let count = hi8(unit0);
            let call_site = CallSiteIdx(u32::from(u16_at(buf, off + 2)));
            let first_reg = u16_at(buf, off + 4);
            Instruction::InvokeCustomRange {
                call_site,
                first_reg,
                count,
            }
        }
        _ => {
            return Err(malformed(
                "invoke opcode",
                off,
                "opcode is outside the decoded family",
            ));
        }
    })
}
