// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::types::instruction::{Instruction, RegList};
use crate::types::method_handle::CallSiteIdx;
use crate::types::{MethodIdx, ProtoIdx};

use super::format::u16_at;

fn hi8(unit: u16) -> u8 {
    (unit >> 8) as u8
}

pub(crate) fn decode_35c_args(count: u8, reg_unit: u16, unit0: u16) -> RegList {
    let c = (reg_unit & 0xF) as u8;
    let d = ((reg_unit >> 4) & 0xF) as u8;
    let e = ((reg_unit >> 8) & 0xF) as u8;
    let f = ((reg_unit >> 12) & 0xF) as u8;
    let g = ((unit0 >> 8) & 0xF) as u8;

    let mut args = RegList::new();
    let regs = [c, d, e, f, g];
    for reg in regs.iter().take(count as usize).copied() {
        args.push(reg);
    }
    args
}

pub fn decode_35c_invoke(buf: &[u8], off: usize, opcode: u8) -> Instruction {
    let unit0 = u16_at(buf, off);
    let count = ((unit0 >> 12) & 0xF) as u8;
    let method = MethodIdx(u16_at(buf, off + 2) as u32);
    let reg_unit = u16_at(buf, off + 4);
    let args = decode_35c_args(count, reg_unit, unit0);

    match opcode {
        0x6e => Instruction::InvokeVirtual { method, args },
        0x6f => Instruction::InvokeSuper { method, args },
        0x70 => Instruction::InvokeDirect { method, args },
        0x71 => Instruction::InvokeStatic { method, args },
        0x72 => Instruction::InvokeInterface { method, args },
        _ => unreachable!(),
    }
}

pub fn decode_3rc_invoke(buf: &[u8], off: usize, opcode: u8) -> Instruction {
    let unit0 = u16_at(buf, off);
    let count = hi8(unit0);
    let method = MethodIdx(u16_at(buf, off + 2) as u32);
    let first_reg = u16_at(buf, off + 4);

    match opcode {
        0x74 => Instruction::InvokeVirtualRange {
            method,
            first_reg,
            count,
        },
        0x75 => Instruction::InvokeSuperRange {
            method,
            first_reg,
            count,
        },
        0x76 => Instruction::InvokeDirectRange {
            method,
            first_reg,
            count,
        },
        0x77 => Instruction::InvokeStaticRange {
            method,
            first_reg,
            count,
        },
        0x78 => Instruction::InvokeInterfaceRange {
            method,
            first_reg,
            count,
        },
        _ => unreachable!(),
    }
}

pub fn decode_invoke_polymorphic(buf: &[u8], off: usize, opcode: u8) -> Instruction {
    let unit0 = u16_at(buf, off);
    match opcode {
        0xfa => {
            let count = ((unit0 >> 12) & 0xF) as u8;
            let method = MethodIdx(u16_at(buf, off + 2) as u32);
            let reg_unit = u16_at(buf, off + 4);
            let proto = ProtoIdx(u16_at(buf, off + 6));
            let args = decode_35c_args(count, reg_unit, unit0);
            Instruction::InvokePolymorphic {
                method,
                proto,
                args,
            }
        }
        0xfb => {
            let count = hi8(unit0);
            let method = MethodIdx(u16_at(buf, off + 2) as u32);
            let first_reg = u16_at(buf, off + 4);
            let proto = ProtoIdx(u16_at(buf, off + 6));
            Instruction::InvokePolymorphicRange {
                method,
                proto,
                first_reg,
                count,
            }
        }
        0xfc => {
            let count = ((unit0 >> 12) & 0xF) as u8;
            let call_site = CallSiteIdx(u16_at(buf, off + 2) as u32);
            let reg_unit = u16_at(buf, off + 4);
            let args = decode_35c_args(count, reg_unit, unit0);
            Instruction::InvokeCustom { call_site, args }
        }
        0xfd => {
            let count = hi8(unit0);
            let call_site = CallSiteIdx(u16_at(buf, off + 2) as u32);
            let first_reg = u16_at(buf, off + 4);
            Instruction::InvokeCustomRange {
                call_site,
                first_reg,
                count,
            }
        }
        _ => unreachable!(),
    }
}
