// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What a decoded A32 or Thumb instruction does to control flow and to the registers that
//! address tracking follows, and the bit fields both encodings are read by.

use super::mode::Mode;

/// A set of general registers, bit `n` standing for `rn`.
pub(super) type Registers = u16;

pub(super) const ALL: Registers = 0xffff;
pub(super) const SP: u8 = 13;
pub(super) const LR: u8 = 14;
pub(super) const PC: u8 = 15;

pub(super) const fn register(index: u8) -> Registers {
    1 << index
}

/// What an instruction does. Its size is the reader's, which knows the encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Instruction {
    /// Whether a condition guards it, in its encoding. A Thumb `it` block guards the
    /// instructions after it instead.
    pub(super) conditional: bool,
    pub(super) flow: Flow,
    pub(super) effect: Effect,
    /// Loads the link register back from the stack without returning, as an epilogue does
    /// before a tail call.
    pub(super) restores_link: bool,
}

impl Instruction {
    pub(super) fn conditional(self) -> Self {
        Self {
            conditional: true,
            ..self
        }
    }

    /// A load of `registers` from `base`, which restores the link register when `base` is the
    /// stack pointer and is written back.
    pub(super) fn load(registers: Registers, base: u8, writeback: bool) -> Self {
        Self {
            restores_link: base == SP && writeback && registers & register(LR) != 0,
            ..Effect::Writes(registers).into()
        }
    }
}

impl From<Flow> for Instruction {
    fn from(flow: Flow) -> Self {
        Self {
            conditional: false,
            flow,
            effect: Effect::None,
            restores_link: false,
        }
    }
}

impl From<Effect> for Instruction {
    fn from(effect: Effect) -> Self {
        Self {
            effect,
            ..Flow::Next.into()
        }
    }
}

/// Where execution goes after an instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Flow {
    /// The next instruction.
    Next,
    /// A branch within the code, which also falls through when conditional.
    Branch(u64),
    /// A call that returns to the next instruction.
    Call(u64, Mode),
    /// A call through a register.
    IndirectCall,
    /// Thumb `tbb` or `tbh`: a branch by a table of byte or halfword offsets right after it.
    Table { base: u64, halfwords: bool },
    /// Thumb `it`, which makes the next `count` instructions conditional.
    IfThen { count: u8 },
    /// A return, a jump through a register or memory, or a trap: the code after it is reached
    /// only from elsewhere.
    Stop,
}

/// What an instruction writes to the registers that address tracking follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Effect {
    /// Writes no general register.
    None,
    /// Writes these registers unknown values.
    Writes(Registers),
    /// Loads `register` with the word at `address`, a literal in the code.
    Literal { register: u8, address: u64 },
    /// Sets `register` to a constant.
    Constant { register: u8, value: u32 },
    /// `movt`: sets the top half of `register`, keeping the bottom one.
    High { register: u8, value: u32 },
    /// Sets `register` to `source` plus the program counter, which reads as `pc`.
    AddPc { register: u8, source: u8, pc: u64 },
    /// Sets `register` to `base` plus a constant.
    Add { register: u8, base: u8, offset: u32 },
    /// A32 `ldr register, [pc, index]`: loads from the program counter, read as `pc`, plus
    /// `index`.
    LoadPcIndexed { register: u8, index: u8, pc: u64 },
}

/// The `bits` bits of `value` from bit `low` up.
pub(super) fn field(value: u32, low: u32, bits: u32) -> u32 {
    (value >> low) & ((1 << bits) - 1)
}

pub(super) fn bit(value: u32, index: u32) -> bool {
    (value >> index) & 1 == 1
}

/// The register numbered by the four bits of `value` from bit `low` up.
pub(super) fn register_at(value: u32, low: u32) -> u8 {
    field(value, low, 4) as u8
}

/// The low `bits` bits of `value` as a signed number.
pub(super) fn sign_extend(value: u32, bits: u32) -> i64 {
    let shift = 32 - bits;
    i64::from(((value << shift) as i32) >> shift)
}

/// The program counter `pc` rounded down to a word, which PC-relative literal loads use.
pub(super) fn word_aligned(pc: u64) -> u64 {
    pc & !3
}
