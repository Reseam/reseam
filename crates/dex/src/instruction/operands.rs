// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use smallvec::SmallVec;

use super::Instruction;
use super::catalogue::instruction_catalogue;
use crate::error::{Result, invalid};

/// What a register holds: a 32-bit value, an object reference, or the low
/// word of a wide value whose high word is the next register.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegisterKind {
    Value,
    Object,
    Wide,
}

impl RegisterKind {
    pub fn words(self) -> u16 {
        if self == Self::Wide { 2 } else { 1 }
    }

    /// The kind of a value of the type a descriptor names.
    pub fn of(descriptor: &str) -> Self {
        match descriptor.as_bytes().first() {
            Some(b'J' | b'D') => Self::Wide,
            Some(b'L' | b'[') => Self::Object,
            _ => Self::Value,
        }
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
    /// `None` where the instruction accepts more than one kind.
    pub kind: Option<RegisterKind>,
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

macro_rules! register_kind {
    (Unknown) => {
        None
    };
    ($kind:ident) => {
        Some(RegisterKind::$kind)
    };
}

macro_rules! visit_arguments {
    (none, $insn:ident, $visit:ident) => {};
    (list, $insn:ident, $visit:ident) => {
        if let InstructionArguments::List(args) = $insn.arguments() {
            for register in args {
                $visit(Operand {
                    register: u16::from(*register),
                    kind: None,
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
                        kind: None,
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
                        kind: None,
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
                    kind: None,
                    access: Access::Read,
                    max: u16::MAX,
                },
                &mut $map,
            )?;
        }
    };
}

macro_rules! define_operands {
    ($($variant:ident [$($shape:tt)*] [$($definition:tt)*] => $opname:ident $opcode:expr, $units:tt; [$($register:ident: $reg_type:ident $kind:ident $access:ident ($max:expr)),*]; $args:ident; [$($index:ident: $id_type:ident $pool:ident $at:literal $index_width:ident),*]; $format:ident($($slot:ident),*);)*) => {
        impl Instruction {
            pub(crate) fn visit_operands(&self, mut visit: impl FnMut(Operand)) {
                match self { $(Self::$variant { $($register,)* .. } => {
                    $(visit(Operand { register: u16::from(*$register), kind: register_kind!($kind), access: Access::$access, max: $max });)*
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
                $(*$register = mapped(Operand { register: u16::from(*$register), kind: register_kind!($kind), access: Access::$access, max: $max }, &mut map)? as $reg_type;)*
                register_arm!($args, insn, $variant, map);
            },)* }
            Ok(())
        }

    };
}
instruction_catalogue!(define_operands);

impl Instruction {
    /// Operand starting registers in encoding order; wide operands occur once.
    pub fn registers_used(&self) -> SmallVec<[u16; 6]> {
        let mut registers = SmallVec::new();
        self.visit_operands(|operand| registers.push(operand.register));
        registers
    }
}

impl Instruction {
    pub fn visit_read_registers(&self, mut visit: impl FnMut(u16)) {
        self.visit_operands(|operand| {
            if operand.access != Access::Write {
                for word in 0..operand.kind.map_or(1, RegisterKind::words) {
                    if let Some(register) = operand.register.checked_add(word) {
                        visit(register);
                    }
                }
            }
        });
    }

    /// Visits value definitions; a check-cast only refines the existing value's type.
    pub fn visit_written_registers(&self, mut visit: impl FnMut(u16)) {
        self.visit_operands(|operand| {
            if operand.access.writes_value() {
                for word in 0..operand.kind.map_or(1, RegisterKind::words) {
                    if let Some(register) = operand.register.checked_add(word) {
                        visit(register);
                    }
                }
            }
        });
    }
}
