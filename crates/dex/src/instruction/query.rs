// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::Instruction;
use super::catalogue::instruction_catalogue;
use crate::model::{FieldIdx, MethodIdx, Pool, StringIdx, TypeIdx};

macro_rules! fixed_width {
    ($units:literal) => {
        $units
    };
    ($variable:tt) => {
        1
    };
}
macro_rules! define_encoding {
    ($($variant:ident [$($shape:tt)*] [$($definition:tt)*] => $opname:ident $opcode:expr, $units:tt; [$($register:ident: $reg_type:ident $kind:ident $access:ident ($max:expr)),*]; $args:ident; [$($index:ident: $id_type:ident $pool:ident $at:literal $index_width:ident),*]; $format:ident($($slot:ident),*);)*) => {
        pub(crate) mod opcodes {
            $(pub(crate) const $opname: u16 = match $opcode { Some(opcode) => opcode, None => u16::MAX };)*
        }
        impl Instruction {
            pub fn code_units(&self) -> u32 {
                match self { $(Self::$variant $($shape)* => $units,)* }
            }
            pub fn opcode(&self) -> Option<u16> {
                match self { $(Self::$variant { .. } => if opcodes::$opname == u16::MAX { None } else { Some(opcodes::$opname) },)* }
            }
        }
        pub(crate) fn opcode_units(opcode: u8) -> Option<usize> {
            const WIDTHS: [u8; 256] = {
                let mut widths = [0; 256];
                $(let opcode: Option<u16> = $opcode; if let Some(opcode) = opcode { if opcode < 256 { widths[opcode as usize] = fixed_width!($units); } })*
                widths
            };
            let width = WIDTHS[usize::from(opcode)];
            (width != 0).then_some(usize::from(width))
        }
    };
}
instruction_catalogue!(define_encoding);

impl Instruction {
    pub fn method_ref(&self) -> Option<MethodIdx> {
        self.index_ref(Pool::Method).map(MethodIdx)
    }
    pub fn field_ref(&self) -> Option<FieldIdx> {
        self.index_ref(Pool::Field).map(FieldIdx)
    }
    pub fn string_ref(&self) -> Option<StringIdx> {
        self.index_ref(Pool::String).map(StringIdx)
    }
    pub fn type_ref(&self) -> Option<TypeIdx> {
        self.index_ref(Pool::Type).map(TypeIdx)
    }

    pub fn literal(&self) -> Option<i64> {
        match self {
            Self::Const4 { value, .. } => Some(i64::from(*value)),
            Self::Const16 { value, .. } | Self::ConstWide16 { value, .. } => {
                Some(i64::from(*value))
            }
            Self::Const { value, .. } | Self::ConstWide32 { value, .. } => Some(i64::from(*value)),
            Self::ConstHigh16 { value, .. } => Some(i64::from(*value) << 16),
            Self::ConstWide { value, .. } => Some(*value),
            Self::ConstWideHigh16 { value, .. } => Some(i64::from(*value) << 48),
            Self::AddIntLit16 { literal, .. }
            | Self::RsubIntLit16 { literal, .. }
            | Self::MulIntLit16 { literal, .. }
            | Self::DivIntLit16 { literal, .. }
            | Self::RemIntLit16 { literal, .. }
            | Self::AndIntLit16 { literal, .. }
            | Self::OrIntLit16 { literal, .. }
            | Self::XorIntLit16 { literal, .. } => Some(i64::from(*literal)),
            Self::AddIntLit8 { literal, .. }
            | Self::RsubIntLit8 { literal, .. }
            | Self::MulIntLit8 { literal, .. }
            | Self::DivIntLit8 { literal, .. }
            | Self::RemIntLit8 { literal, .. }
            | Self::AndIntLit8 { literal, .. }
            | Self::OrIntLit8 { literal, .. }
            | Self::XorIntLit8 { literal, .. }
            | Self::ShlIntLit8 { literal, .. }
            | Self::ShrIntLit8 { literal, .. }
            | Self::UshrIntLit8 { literal, .. } => Some(i64::from(*literal)),
            _ => None,
        }
    }

    pub fn is_invoke(&self) -> bool {
        matches!(self.opcode(), Some(0x6e..=0x72 | 0x74..=0x78 | 0xfa..=0xfd))
    }
    pub fn is_return(&self) -> bool {
        matches!(self.opcode(), Some(0x0e..=0x11))
    }
}

macro_rules! define_indices {
    ($($variant:ident [$($shape:tt)*] [$($definition:tt)*] => $opname:ident $opcode:expr, $units:tt; [$($register:ident: $reg_type:ident $kind:ident $access:ident ($max:expr)),*]; $args:ident; [$($index:ident: $id_type:ident $pool:ident $at:literal $index_width:ident),*]; $format:ident($($slot:ident),*);)*) => {
        impl Instruction {
            pub(crate) fn index_ref(&self, pool: Pool) -> Option<u32> {
                match self { $(Self::$variant { $($index,)* .. } => {
                    $(if pool == Pool::$pool { return Some($index.0); })*
                    None
                },)* }
            }
            pub(crate) fn indices(&self) -> smallvec::SmallVec<[(Pool, u32); 2]> {
                match self { $(Self::$variant { $($index,)* .. } => smallvec::smallvec![$((Pool::$pool, $index.0),)*],)* }
            }
            pub(crate) fn map_indices(&mut self, map: impl Fn(Pool, u32) -> u32) {
                match self { $(Self::$variant { $($index,)* .. } => {
                    $($index.0 = map(Pool::$pool, $index.0);)*
                },)* }
            }
        }
    };
}
instruction_catalogue!(define_indices);
