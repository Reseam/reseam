// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! One decoder and one encoder per instruction format; the catalogue names
//! each instruction's format and which of its fields fills each slot.

mod decode;
mod encode;
mod payload;

pub use decode::decode_instructions;
pub use encode::encode_instructions;

/// Whether instructions of a format carry a literal.
macro_rules! literal_format {
    (f11n) => {
        true
    };
    (f21s) => {
        true
    };
    (f21h) => {
        true
    };
    (f31i) => {
        true
    };
    (f51l) => {
        true
    };
    (f22b) => {
        true
    };
    (f22s) => {
        true
    };
    ($format:ident) => {
        false
    };
}

macro_rules! define_literals {
    ($($variant:ident [$($shape:tt)*] [$($definition:tt)*] => $opname:ident $opcode:pat, $units:tt; [$($register:ident: $reg_type:ident $kind:ident $access:ident ($max:expr)),*]; $args:ident; [$($index:ident: $id_type:ident $pool:ident $at:literal $index_width:ident),*]; $format:ident($($slot:ident),*);)*) => {
        /// Whether an opcode's instructions carry a literal.
        pub(crate) fn has_literal(opcode: u8) -> bool {
            const LITERALS: [bool; 256] = {
                let mut literals = [false; 256];
                $(if super::opcodes::$opname < 256 {
                    literals[super::opcodes::$opname as usize] = literal_format!($format);
                })*
                literals
            };
            LITERALS[usize::from(opcode)]
        }
    };
}
super::catalogue::instruction_catalogue!(define_literals);
