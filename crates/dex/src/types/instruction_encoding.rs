// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::instruction::Instruction;
use super::instruction_catalogue::instruction_catalogue;

macro_rules! fixed_width {
    ($units:literal) => {
        $units
    };
    ($variable:tt) => {
        1
    };
}
macro_rules! define_encoding {
    ($($variant:ident [$($shape:tt)*] [$($definition:tt)*] => $opname:ident $opcode:expr, $units:tt; [$($register:ident: $reg_type:ident $kind:ident $access:ident ($max:expr)),*]; $args:ident; [$($index:ident: $id_type:ident $pool:ident $at:literal $index_width:ident),*];)*) => {
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
    /// Argument words passed to invokes and filled-new-array.
    pub fn outgoing_arg_count(&self) -> u16 {
        self.argument_words()
    }
}
