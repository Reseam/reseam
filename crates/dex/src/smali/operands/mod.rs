// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Operands in smali order, one function per Dalvik instruction format. The
//! catalogue names each variant's format and its fields in format order.

mod formats;
mod payloads;

use super::body::Body;
use super::names::SmaliNames;
use crate::error::Result;
use crate::instruction::Instruction;
use crate::instruction::catalogue::instruction_catalogue;
use crate::model::{
    CallSiteIdx, FieldIdx, MethodHandleIdx, MethodIdx, ProtoIdx, StringIdx, TypeIdx,
};
use formats::{
    f3rc, f4rcc, f10t, f10x, f11n, f11x, f12x, f20t, f21c, f21h, f21s, f21t, f22b, f22c, f22s,
    f22t, f22x, f23x, f30t, f31c, f31i, f31t, f32x, f35c, f45cc, f51l, nop,
};
use payloads::{payload, raw};

/// A register number or register count, as wide as its encoding.
pub(super) trait Unsigned: Copy {
    fn get(self) -> u16;
}

impl Unsigned for u8 {
    fn get(self) -> u16 {
        u16::from(self)
    }
}

impl Unsigned for u16 {
    fn get(self) -> u16 {
        self
    }
}

pub(super) trait Delta: Copy {
    fn delta(self) -> i32;
}

impl Delta for i8 {
    fn delta(self) -> i32 {
        i32::from(self)
    }
}

impl Delta for i16 {
    fn delta(self) -> i32 {
        i32::from(self)
    }
}

impl Delta for i32 {
    fn delta(self) -> i32 {
        self
    }
}

pub(super) trait Reference {
    fn name(&self, names: SmaliNames<'_>) -> Result<String>;
}

macro_rules! plain_references {
    ($($idx:ident => $method:ident),*) => {$(
        impl Reference for $idx {
            fn name(&self, names: SmaliNames<'_>) -> Result<String> {
                Ok(names.$method(*self))
            }
        }
    )*};
}
plain_references!(StringIdx => string, TypeIdx => type_, FieldIdx => field, MethodIdx => method, ProtoIdx => proto);

impl Reference for MethodHandleIdx {
    fn name(&self, names: SmaliNames<'_>) -> Result<String> {
        names.method_handle(*self)
    }
}

impl Reference for CallSiteIdx {
    fn name(&self, names: SmaliNames<'_>) -> Result<String> {
        names.call_site(*self)
    }
}

/// A format's operands, from formats that cannot fail and from those that
/// name pool items, which can.
trait IntoOperands {
    fn into_operands(self) -> Result<String>;
}

impl IntoOperands for String {
    fn into_operands(self) -> Result<String> {
        Ok(self)
    }
}

impl IntoOperands for Result<String> {
    fn into_operands(self) -> Result<String> {
        self
    }
}

macro_rules! define_operands {
    ($($variant:ident [$($shape:tt)*] [$($definition:tt)*] => $opname:ident $opcode:expr, $units:tt; [$($register:ident: $reg_type:ident $kind:ident $access:ident ($max:expr)),*]; $args:ident; [$($index:ident: $id_type:ident $pool:ident $at:literal $index_width:ident),*]; $format:ident($($slot:ident),*);)*) => {
        impl Body<'_> {
            /// The operands of instruction `at`, as smali writes them after the mnemonic.
            pub(super) fn operands(&self, at: usize, insn: &Instruction) -> Result<String> {
                match insn {
                    $(Instruction::$variant { $($slot,)* .. } => $format(self, at, insn, $($slot),*).into_operands(),)*
                }
            }
        }

        /// The catalogue's name for the instruction's opcode.
        pub(super) fn opname(insn: &Instruction) -> &'static str {
            match insn {
                $(Instruction::$variant { .. } => stringify!($opname),)*
            }
        }
    };
}
instruction_catalogue!(define_operands);
