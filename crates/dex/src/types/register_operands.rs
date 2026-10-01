// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::instruction::Instruction;
use super::instruction_catalogue::instruction_catalogue;
use crate::error::{Result, invalid};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RegisterKind {
    Value,
    Object,
    Wide,
    Unknown,
}

impl RegisterKind {
    pub fn words(self) -> u16 {
        if self == Self::Wide { 2 } else { 1 }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Access {
    Read,
    Write,
    ReadWrite,
    Refine,
}

pub(crate) struct Operand {
    pub register: u16,
    pub kind: RegisterKind,
    pub access: Access,
    pub max: u16,
}

impl Access {
    pub(crate) fn writes_value(self) -> bool {
        matches!(self, Self::Write | Self::ReadWrite)
    }
}

fn mapped(operand: Operand, map: &mut impl FnMut(Operand) -> Result<u16>) -> Result<u16> {
    let max = operand.max;
    let register = map(operand)?;
    if register > max {
        return Err(invalid(
            "register operand",
            "mapped register exceeds operand width",
        ));
    }
    Ok(register)
}

macro_rules! visit_arguments {
    (none, $insn:ident, $visit:ident) => {};
    (list, $insn:ident, $visit:ident) => {
        if let InstructionArguments::List(args) = $insn.arguments() {
            for register in args {
                $visit(Operand {
                    register: u16::from(*register),
                    kind: RegisterKind::Unknown,
                    access: Access::Read,
                    max: 15,
                });
            }
        }
    };
    (range, $insn:ident, $visit:ident) => {
        if let InstructionArguments::Range { first, count } = $insn.arguments() {
            for register in u32::from(first)..u32::from(first) + u32::from(count) {
                if let Ok(register) = u16::try_from(register) {
                    $visit(Operand {
                        register,
                        kind: RegisterKind::Unknown,
                        access: Access::Read,
                        max: u16::MAX,
                    });
                }
            }
        }
    };
}

pub(crate) enum InstructionArguments<'a> {
    None,
    List(&'a [u8]),
    Range { first: u16, count: u8 },
}

macro_rules! arguments {
    (none, $insn:ident, $variant:ident) => {
        InstructionArguments::None
    };
    (list, $insn:ident, $variant:ident) => {
        match $insn {
            Instruction::$variant { args, .. } => InstructionArguments::List(args),
            _ => unreachable!("catalogue dispatch has already matched this instruction variant"),
        }
    };
    (range, $insn:ident, $variant:ident) => {
        match $insn {
            Instruction::$variant {
                first_reg, count, ..
            } => InstructionArguments::Range {
                first: *first_reg,
                count: *count,
            },
            _ => unreachable!("catalogue dispatch has already matched this instruction variant"),
        }
    };
}

macro_rules! register_arm {
    (none, $insn:ident, $variant:ident, $map:ident) => {};
    (list, $insn:ident, $variant:ident, $map:ident) => {
        if let Instruction::$variant { args, .. } = $insn {
            for register in args.iter_mut() {
                *register = mapped(
                    Operand {
                        register: u16::from(*register),
                        kind: RegisterKind::Unknown,
                        access: Access::Read,
                        max: 15,
                    },
                    &mut $map,
                )? as u8;
            }
        }
    };
    (range, $insn:ident, $variant:ident, $map:ident) => {
        if let Instruction::$variant {
            first_reg, count, ..
        } = $insn
            && *count != 0
        {
            *first_reg = mapped(
                Operand {
                    register: *first_reg,
                    kind: RegisterKind::Unknown,
                    access: Access::Read,
                    max: u16::MAX,
                },
                &mut $map,
            )?;
        }
    };
}

macro_rules! define_operands {
    ($($variant:ident [$($shape:tt)*] [$($definition:tt)*] => $opname:ident $opcode:expr, $units:tt; [$($register:ident: $reg_type:ident $kind:ident $access:ident ($max:expr)),*]; $args:ident; [$($index:ident: $id_type:ident $pool:ident $at:literal $index_width:ident),*];)*) => {
        impl Instruction {
            pub(crate) fn visit_operands(&self, mut visit: impl FnMut(Operand)) {
                match self { $(Self::$variant { $($register,)* .. } => {
                    $(visit(Operand { register: u16::from(*$register), kind: RegisterKind::$kind, access: Access::$access, max: $max });)*
                    visit_arguments!($args, self, visit);
                },)* }
            }

            pub(crate) fn arguments(&self) -> InstructionArguments<'_> {
                match self { $(Self::$variant { .. } => arguments!($args, self, $variant),)* }
            }

            pub(crate) fn argument_words(&self) -> u16 {
                match self.arguments() {
                    InstructionArguments::None => 0,
                    InstructionArguments::List(args) => args.len() as u16,
                    InstructionArguments::Range { count, .. } => u16::from(count),
                }
            }
        }

        pub(crate) fn map_operands(insn: &mut Instruction, mut map: impl FnMut(Operand) -> Result<u16>) -> Result<()> {
            match insn { $(Instruction::$variant { $($register,)* .. } => {
                $(*$register = mapped(Operand { register: u16::from(*$register), kind: RegisterKind::$kind, access: Access::$access, max: $max }, &mut map)? as $reg_type;)*
                register_arm!($args, insn, $variant, map);
            },)* }
            Ok(())
        }

    };
}
instruction_catalogue!(define_operands);
