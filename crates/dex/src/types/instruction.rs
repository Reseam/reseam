// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::method_handle::{CallSiteIdx, MethodHandleIdx};
use super::{FieldIdx, MethodIdx, ProtoIdx, StringIdx, TypeIdx};

/// At most five register arguments for a compact instruction.
/// Dynamic lists use fallible construction; longer invocations require a range form.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RegList {
    registers: [u8; 5],
    len: u8,
}

impl RegList {
    /// Returns an error when the list exceeds the compact format's five registers.
    pub fn try_from_iter(registers: impl IntoIterator<Item = u8>) -> crate::Result<Self> {
        let mut args = Self::default();
        for register in registers {
            let slot = args
                .registers
                .get_mut(usize::from(args.len))
                .ok_or_else(|| {
                    crate::error::invalid("compact arguments", "more than five registers")
                })?;
            *slot = register;
            args.len += 1;
        }
        Ok(args)
    }
}

impl TryFrom<&[u8]> for RegList {
    type Error = crate::DexError;

    fn try_from(registers: &[u8]) -> crate::Result<Self> {
        Self::try_from_iter(registers.iter().copied())
    }
}

impl std::ops::Deref for RegList {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.registers[..usize::from(self.len)]
    }
}

impl std::ops::DerefMut for RegList {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.registers[..usize::from(self.len)]
    }
}

impl IntoIterator for RegList {
    type Item = u8;
    type IntoIter = std::iter::Take<std::array::IntoIter<u8, 5>>;

    fn into_iter(self) -> Self::IntoIter {
        self.registers.into_iter().take(usize::from(self.len))
    }
}

macro_rules! array_arguments {
    ($($len:literal),*) => {$(
        impl From<[u8; $len]> for RegList {
            fn from(registers: [u8; $len]) -> Self {
                let mut args = Self::default();
                args.registers[..$len].copy_from_slice(&registers);
                args.len = $len;
                args
            }
        }
    )*};
}
array_arguments!(0, 1, 2, 3, 4, 5);

/// Payload of a `packed-switch`, boxed so the rare variant does not widen the
/// common `Instruction` cases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackedSwitchData {
    pub first_key: i32,
    pub targets: Vec<i32>,
}

/// Payload of a `sparse-switch`, boxed for the same reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SparseSwitchData {
    pub keys_and_targets: Vec<(i32, i32)>,
}

/// Payload of a `fill-array-data`, boxed for the same reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FillArrayPayloadData {
    pub element_width: u16,
    pub data: Vec<u8>,
}

macro_rules! define_instructions {
    ($($variant:ident [$($shape:tt)*] [$($definition:tt)*] => $opname:ident $opcode:expr, $units:tt; [$($register:ident: $reg_type:ident $kind:ident $access:ident ($max:expr)),*]; $args:ident; [$($index:ident: $id_type:ident $pool:ident $at:literal $index_width:ident),*];)*) => {
        /// DEX instructions. Register and index widths are checked during serialization;
        /// opaque code units are retained exactly when no supported opcode describes them.
        #[derive(Debug, Clone, PartialEq, Eq)]
        #[non_exhaustive]
        pub enum Instruction { $($variant $($definition)*,)* }
    };
}
super::instruction_catalogue::instruction_catalogue!(define_instructions);
