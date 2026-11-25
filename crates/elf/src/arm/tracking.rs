// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Following the values registers hold, to find the addresses code computes. Position-independent
//! code loads an address's distance from an instruction, as a literal or with `movw` and `movt`,
//! then adds the program counter there. Registers are 32 bits wide, so their values wrap there.

use reseam_bytes::u32_at;

use super::instruction::{Effect, Registers, register as register_bit};
use crate::file::ElfImage;

/// The registers a procedure call may overwrite, per AAPCS: the argument registers r0-r3, the
/// scratch register r12 and the link register.
const CALLER_SAVED: Registers = 0b0101_0000_0000_1111;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Value {
    /// A number, such as an address's distance from the program counter.
    Constant(u32),
    /// An address in the library.
    Address(u32),
}

/// An address an instruction computes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Computed {
    /// The program counter plus a distance: a pointer the code materializes, to data or to a
    /// function.
    Pointer(u64),
    /// A computed address plus a constant, such as a field or a string inside a larger block.
    Offset(u64),
    /// Read from memory.
    Read(u64),
}

/// What each general register is known to hold: a value where `known` has its bit, an address
/// where `addresses` does too.
#[derive(Debug, Default, Clone)]
pub(super) struct Tracker {
    values: [u32; 16],
    known: Registers,
    addresses: Registers,
}

impl Tracker {
    /// Applies an instruction's `effect`, returning the address it computes, if any.
    pub(super) fn apply(&mut self, image: &ElfImage, effect: Effect) -> Option<Computed> {
        let (target, value) = match effect {
            Effect::None => return None,
            Effect::Writes(registers) => {
                self.forget(registers);
                return None;
            }
            Effect::Literal { register, address } => (
                register,
                image
                    .loaded(address, 4)
                    .ok()
                    .map(|bytes| Value::Constant(u32_at(bytes, 0))),
            ),
            Effect::Constant { register, value } => (register, Some(Value::Constant(value))),
            Effect::High { register, value } => (
                register,
                match self.get(register) {
                    Some(Value::Constant(low)) => Some(Value::Constant(low & 0xffff | value << 16)),
                    _ => None,
                },
            ),
            Effect::AddPc {
                register,
                source,
                pc,
            } => (
                register,
                match self.get(source) {
                    Some(Value::Constant(distance)) => {
                        Some(Value::Address(distance.wrapping_add(pc as u32)))
                    }
                    _ => None,
                },
            ),
            Effect::Add {
                register,
                base,
                offset,
            } => (
                register,
                self.get(base).map(|value| match value {
                    Value::Constant(number) => Value::Constant(number.wrapping_add(offset)),
                    Value::Address(address) => Value::Address(address.wrapping_add(offset)),
                }),
            ),
            Effect::LoadPcIndexed {
                register,
                index,
                pc,
            } => {
                let read = match self.get(index) {
                    Some(Value::Constant(distance)) => Some(distance.wrapping_add(pc as u32)),
                    _ => None,
                };
                self.forget(register_bit(register));
                return read.map(|address| Computed::Read(u64::from(address)));
            }
        };
        self.set(target, value);
        let address = match value {
            Some(Value::Address(address)) => u64::from(address),
            _ => return None,
        };
        Some(match effect {
            Effect::AddPc { .. } => Computed::Pointer(address),
            _ => Computed::Offset(address),
        })
    }

    /// Forgets what a procedure call may overwrite.
    pub(super) fn call(&mut self) {
        self.forget(CALLER_SAVED);
    }

    fn get(&self, index: u8) -> Option<Value> {
        let bit = register_bit(index);
        let value = self.values[usize::from(index)];
        match (self.known & bit != 0, self.addresses & bit != 0) {
            (false, _) => None,
            (true, false) => Some(Value::Constant(value)),
            (true, true) => Some(Value::Address(value)),
        }
    }

    fn set(&mut self, index: u8, value: Option<Value>) {
        let bit = register_bit(index);
        self.forget(bit);
        if let Some(Value::Constant(value) | Value::Address(value)) = value {
            self.values[usize::from(index)] = value;
            self.known |= bit;
        }
        if let Some(Value::Address(_)) = value {
            self.addresses |= bit;
        }
    }

    fn forget(&mut self, registers: Registers) {
        self.known &= !registers;
        self.addresses &= !registers;
    }
}
