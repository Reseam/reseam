// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::types::instruction_catalogue::instruction_catalogue;

macro_rules! define_opcode_matchers {
    ($($variant:ident [$($shape:tt)*] [$($definition:tt)*] => $opname:ident $opcode:expr, $units:tt; [$($register:ident: $reg_type:ident $kind:ident $access:ident ($max:expr)),*]; $args:ident; [$($index:ident: $id_type:ident $pool:ident $at:literal $index_width:ident),*];)*) => {
        #[derive(Debug, Clone, Copy)]
        pub enum OpcodeMatcher { $($variant,)* }
        impl OpcodeMatcher {
            pub(super) const fn opcode(self) -> Option<u16> {
                match self { $(Self::$variant => $opcode,)* }
            }
        }
    };
}
instruction_catalogue!(define_opcode_matchers);
