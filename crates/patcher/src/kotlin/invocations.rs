// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Bridge invocations lowered through the DEX invoke rules: argument moves
//! into scratch registers and the encoding that reaches the arguments.

use reseam_dex::{
    ArgumentMove, Invocation, InvokeEncoding, InvokeForm, Receiver, RegisterKind, argument_kinds,
    array_kinds,
};

use super::convert::invalid;
use super::types::{
    CustomRangeInsn, FilledArrayRangeInsn, Instruction, InvokeRangeInsn, PolymorphicRangeInsn,
    Reg2Insn,
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

/// Scratch words an instruction needs to be lowered; zero for all but
/// invocations whose arguments no encoding reaches where they are.
pub(super) fn scratch_words(instruction: &Instruction) -> reseam_dex::Result<u16> {
    Ok(match invocation(instruction)? {
        Some(written) => written.invocation()?.scratch_words(),
        None => 0,
    })
}

/// The instructions an invocation lowers to: copies into `scratch` when its
/// arguments are not consecutive, then the call in the form that reaches
/// them. Any other instruction lowers to itself.
pub(super) fn lower_invocation(
    instruction: Instruction,
    scratch: &[u16],
) -> reseam_dex::Result<Vec<Instruction>> {
    let Some(written) = invocation(&instruction)? else {
        return Ok(vec![instruction]);
    };
    let invocation = written.invocation()?;
    let words = usize::from(invocation.scratch_words());
    if words != 0
        && (scratch.len() != words
            || scratch
                .windows(2)
                .any(|pair| pair[0].checked_add(1) != Some(pair[1])))
    {
        return Err(invalid(
            "scratch span must be consecutive and match the argument words",
        ));
    }
    let (moves, encoding) = invocation.lower(scratch.first().copied().filter(|_| words != 0))?;
    let mut lowered: Vec<Instruction> = moves.into_iter().map(move_argument).collect();
    lowered.push(match encoding {
        InvokeEncoding::Compact => instruction,
        InvokeEncoding::Range { first, count } => range(instruction, first, u16::from(count))?,
    });
    Ok(lowered)
}

fn move_argument(argument: ArgumentMove) -> Instruction {
    let base = match argument.kind {
        RegisterKind::Value => 1,
        RegisterKind::Wide => 4,
        RegisterKind::Object => 7,
    };
    let width = if argument.dest <= 15 && argument.src <= 15 {
        0
    } else if argument.dest <= 255 {
        1
    } else {
        2
    };
    Instruction::Reg2(Reg2Insn {
        opcode: base + width,
        reg_a: argument.dest,
        reg_b: argument.src,
    })
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
