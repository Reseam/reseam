// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Bridge invocations encoded through the DEX invoke rules.

use reseam_dex::{
    Invocation, InvokeEncoding, InvokeForm, Receiver, RegisterKind, argument_kinds, array_kinds,
};

use super::convert::invalid;
use super::types::{
    CustomRangeInsn, FilledArrayRangeInsn, Instruction, InvokeRangeInsn, PolymorphicRangeInsn,
};

/// An invoke or filled-new-array as written: its form, its argument words,
/// and the kinds of the arguments they carry.
struct Written {
    form: InvokeForm,
    registers: Vec<u16>,
    kinds: Vec<RegisterKind>,
}

impl Written {
    fn invocation(&self) -> reseam_dex::Result<Invocation<'_>> {
        Invocation::new(self.form, &self.registers, &self.kinds)
    }
}

/// `None` for an instruction that is no invocation.
fn invocation(instruction: &Instruction) -> reseam_dex::Result<Option<Written>> {
    let receiver = |is_static| {
        if is_static {
            Receiver::Absent
        } else {
            Receiver::Present
        }
    };
    Ok(Some(match instruction {
        Instruction::Invoke(value) if (0x6e..=0x72).contains(&value.opcode) => Written {
            form: InvokeForm::Compact,
            registers: value.registers.clone(),
            kinds: argument_kinds(&value.method.proto, receiver(value.opcode == 0x71))?,
        },
        Instruction::InvokeRange(value) if (0x74..=0x78).contains(&value.opcode) => Written {
            form: InvokeForm::Range,
            registers: range_registers(value.start_reg, value.reg_count)?,
            kinds: argument_kinds(&value.method.proto, receiver(value.opcode == 0x77))?,
        },
        Instruction::Polymorphic(value) if value.opcode == 0xfa => Written {
            form: InvokeForm::Compact,
            registers: value.registers.clone(),
            kinds: argument_kinds(&value.proto, Receiver::Present)?,
        },
        Instruction::PolymorphicRange(value) if value.opcode == 0xfb => Written {
            form: InvokeForm::Range,
            registers: range_registers(value.start_reg, value.reg_count)?,
            kinds: argument_kinds(&value.proto, Receiver::Present)?,
        },
        Instruction::Custom(value) if value.opcode == 0xfc => Written {
            form: InvokeForm::Compact,
            registers: value.registers.clone(),
            kinds: argument_kinds(&value.call_site.proto, Receiver::Absent)?,
        },
        Instruction::CustomRange(value) if value.opcode == 0xfd => Written {
            form: InvokeForm::Range,
            registers: range_registers(value.start_reg, value.reg_count)?,
            kinds: argument_kinds(&value.call_site.proto, Receiver::Absent)?,
        },
        Instruction::FilledArray(value) if value.opcode == 0x24 => Written {
            form: InvokeForm::Compact,
            registers: value.registers.clone(),
            kinds: array_kinds(&value.type_descriptor, value.registers.len())?,
        },
        Instruction::FilledArrayRange(value) if value.opcode == 0x25 => Written {
            form: InvokeForm::Range,
            registers: range_registers(value.start_reg, value.reg_count)?,
            kinds: array_kinds(&value.type_descriptor, usize::from(value.reg_count))?,
        },
        Instruction::Invoke(_)
        | Instruction::InvokeRange(_)
        | Instruction::Polymorphic(_)
        | Instruction::PolymorphicRange(_)
        | Instruction::Custom(_)
        | Instruction::CustomRange(_)
        | Instruction::FilledArray(_)
        | Instruction::FilledArrayRange(_) => {
            return Err(invalid("opcode does not match invocation shape"));
        }
        _ => return Ok(None),
    }))
}

fn range_registers(start: u16, count: u16) -> reseam_dex::Result<Vec<u16>> {
    (u32::from(start)..u32::from(start) + u32::from(count))
        .map(|register| u16::try_from(register).map_err(|_| invalid("range exceeds v65535")))
        .collect()
}

/// An invocation in the form that reaches its arguments where they are: the form it was
/// written in when that fits, else the range form. Any other instruction is unchanged.
pub(in crate::bridge) fn encoded_invocation(
    instruction: Instruction,
) -> reseam_dex::Result<Instruction> {
    let Some(written) = invocation(&instruction)? else {
        return Ok(instruction);
    };
    match written.invocation()?.lower(None)?.1 {
        InvokeEncoding::Compact => Ok(instruction),
        InvokeEncoding::Range { first, count } => range(instruction, first, u16::from(count)),
    }
}

/// The range form of an invocation, reading `count` words from `first`.
fn range(instruction: Instruction, first: u16, count: u16) -> reseam_dex::Result<Instruction> {
    Ok(match instruction {
        Instruction::Invoke(value) => Instruction::InvokeRange(InvokeRangeInsn {
            opcode: value.opcode + 6,
            start_reg: first,
            reg_count: count,
            method: value.method,
        }),
        Instruction::InvokeRange(value) => Instruction::InvokeRange(InvokeRangeInsn {
            start_reg: first,
            reg_count: count,
            ..value
        }),
        Instruction::Polymorphic(value) => Instruction::PolymorphicRange(PolymorphicRangeInsn {
            opcode: 0xfb,
            start_reg: first,
            reg_count: count,
            method: value.method,
            proto: value.proto,
        }),
        Instruction::PolymorphicRange(value) => {
            Instruction::PolymorphicRange(PolymorphicRangeInsn {
                start_reg: first,
                reg_count: count,
                ..value
            })
        }
        Instruction::Custom(value) => Instruction::CustomRange(CustomRangeInsn {
            opcode: 0xfd,
            start_reg: first,
            reg_count: count,
            call_site: value.call_site,
        }),
        Instruction::CustomRange(value) => Instruction::CustomRange(CustomRangeInsn {
            start_reg: first,
            reg_count: count,
            ..value
        }),
        Instruction::FilledArray(value) => Instruction::FilledArrayRange(FilledArrayRangeInsn {
            opcode: 0x25,
            start_reg: first,
            reg_count: count,
            type_descriptor: value.type_descriptor,
        }),
        Instruction::FilledArrayRange(value) => {
            Instruction::FilledArrayRange(FilledArrayRangeInsn {
                start_reg: first,
                reg_count: count,
                ..value
            })
        }
        _ => return Err(invalid("range form requires an invocation")),
    })
}
