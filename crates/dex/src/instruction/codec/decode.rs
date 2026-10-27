// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::error::{Result, malformed};
use crate::instruction::catalogue::instruction_catalogue;
use crate::instruction::stream::{RawInstruction, instruction_stream};
use crate::instruction::{Instruction, RegList};
use reseam_bytes::{u16_at, u32_at};
use smallvec::SmallVec;

/// Decodes `insns_size` code units starting at byte `start` of `buf`.
/// Truncated or malformed encodings fail; unknown opcodes keep all their
/// original code units. Pool indices are resolved by the owning DEX file.
pub fn decode_instructions(
    buf: &[u8],
    start: usize,
    insns_size: usize,
) -> Result<Vec<Instruction>> {
    let mut instructions = Vec::with_capacity(insns_size.min(buf.len().saturating_sub(start) / 2));
    for frame in instruction_stream(buf, start, insns_size)? {
        let frame = frame?;
        decode_onto(buf, &frame, &mut instructions)?;
        debug_assert_eq!(
            Some(frame.units),
            instructions.last().map(|insn| insn.code_units() as usize)
        );
    }
    Ok(instructions)
}

impl RawInstruction {
    /// The literal of a `const*` or `*-int/lit*` instruction, as
    /// [`Instruction::literal`] reports it once decoded.
    pub fn literal(&self, buf: &[u8]) -> Option<i64> {
        if !super::has_literal(self.opcode) {
            return None;
        }
        let mut decoded = SmallVec::<[Instruction; 1]>::new();
        decode_onto(buf, self, &mut decoded).ok()?;
        decoded.pop()?.literal()
    }
}

/// The code units of one framed instruction.
#[derive(Clone, Copy)]
pub(crate) struct Units<'a> {
    buf: &'a [u8],
    at: usize,
    len: usize,
}

impl Units<'_> {
    fn unit(self, index: usize) -> u16 {
        u16_at(self.buf, self.at + index * 2)
    }

    fn word(self, index: usize) -> u32 {
        u32_at(self.buf, self.at + index * 2)
    }

    fn a(self) -> u8 {
        (self.unit(0) >> 8) as u8 & 0xf
    }

    fn b(self) -> u8 {
        (self.unit(0) >> 12) as u8
    }

    fn aa(self) -> u8 {
        (self.unit(0) >> 8) as u8
    }

    fn signed_b(self) -> i8 {
        (self.unit(0) >> 8) as i8 >> 4
    }

    fn long(self) -> i64 {
        i64::from(self.word(1)) | i64::from(self.word(3)) << 32
    }

    /// The arguments of a 35c or 45cc instruction: A of them from C, D, E, F, G.
    fn args(self) -> Result<RegList> {
        let count = usize::from(self.b());
        if count > 5 {
            return Err(malformed(
                "compact instruction",
                self.at,
                "argument count exceeds five registers",
            ));
        }
        let unit = self.unit(2);
        let [c, d, e, f] = [0, 4, 8, 12].map(|shift| (unit >> shift) as u8 & 0xf);
        Ok(RegList::new([c, d, e, f, self.a()], count as u8))
    }

    fn raw(self) -> Instruction {
        let at = self.at;
        Instruction::Raw {
            code_units: (0..self.len)
                .map(|index| u16_at(self.buf, at + index * 2))
                .collect(),
        }
    }

    /// A nop, or the payload pseudo-instruction whose identifier it starts.
    fn nop_or_payload(self) -> Result<Instruction> {
        Ok(match self.unit(0) {
            0 => Instruction::Nop,
            _ => super::payload::decode(self.buf, self.at)?.unwrap_or_else(|| self.raw()),
        })
    }
}

macro_rules! decode_format {
    (nop, $u:ident, $v:ident) => { $u.nop_or_payload()? };
    (payload, $u:ident, $v:ident) => { $u.nop_or_payload()? };
    (raw, $u:ident, $v:ident) => { $u.raw() };
    (f10x, $u:ident, $v:ident) => { Instruction::$v {} };
    (f12x, $u:ident, $v:ident, $a:ident, $b:ident) => { Instruction::$v { $a: $u.a(), $b: $u.b() } };
    (f11n, $u:ident, $v:ident, $a:ident, $b:ident) => { Instruction::$v { $a: $u.a(), $b: $u.signed_b() } };
    (f11x, $u:ident, $v:ident, $a:ident) => { Instruction::$v { $a: $u.aa() } };
    (f10t, $u:ident, $v:ident, $a:ident) => { Instruction::$v { $a: $u.aa() as i8 } };
    (f20t, $u:ident, $v:ident, $a:ident) => { Instruction::$v { $a: $u.unit(1) as i16 } };
    (f30t, $u:ident, $v:ident, $a:ident) => { Instruction::$v { $a: $u.word(1) as i32 } };
    (f22x, $u:ident, $v:ident, $a:ident, $b:ident) => { Instruction::$v { $a: $u.aa(), $b: $u.unit(1) } };
    (f32x, $u:ident, $v:ident, $a:ident, $b:ident) => { Instruction::$v { $a: $u.unit(1), $b: $u.unit(2) } };
    (f21t, $($rest:tt)*) => { decode_format!(f21s, $($rest)*) };
    (f21h, $($rest:tt)*) => { decode_format!(f21s, $($rest)*) };
    (f21s, $u:ident, $v:ident, $a:ident, $b:ident) => { Instruction::$v { $a: $u.aa(), $b: $u.unit(1) as i16 } };
    (f21c, $u:ident, $v:ident, $a:ident, $b:ident) => {
        Instruction::$v { $a: $u.aa(), $b: u32::from($u.unit(1)).into() }
    };
    (f23x, $u:ident, $v:ident, $a:ident, $b:ident, $c:ident) => {
        Instruction::$v { $a: $u.aa(), $b: $u.unit(1) as u8, $c: ($u.unit(1) >> 8) as u8 }
    };
    (f22b, $u:ident, $v:ident, $a:ident, $b:ident, $c:ident) => {
        Instruction::$v { $a: $u.aa(), $b: $u.unit(1) as u8, $c: ($u.unit(1) >> 8) as i8 }
    };
    (f22t, $($rest:tt)*) => { decode_format!(f22s, $($rest)*) };
    (f22s, $u:ident, $v:ident, $a:ident, $b:ident, $c:ident) => {
        Instruction::$v { $a: $u.a(), $b: $u.b(), $c: $u.unit(1) as i16 }
    };
    (f22c, $u:ident, $v:ident, $a:ident, $b:ident, $c:ident) => {
        Instruction::$v { $a: $u.a(), $b: $u.b(), $c: u32::from($u.unit(1)).into() }
    };
    (f31t, $($rest:tt)*) => { decode_format!(f31i, $($rest)*) };
    (f31i, $u:ident, $v:ident, $a:ident, $b:ident) => { Instruction::$v { $a: $u.aa(), $b: $u.word(1) as i32 } };
    (f31c, $u:ident, $v:ident, $a:ident, $b:ident) => { Instruction::$v { $a: $u.aa(), $b: $u.word(1).into() } };
    (f51l, $u:ident, $v:ident, $a:ident, $b:ident) => { Instruction::$v { $a: $u.aa(), $b: $u.long() } };
    (f35c, $u:ident, $v:ident, $index:ident, $args:ident) => {
        Instruction::$v { $index: u32::from($u.unit(1)).into(), $args: $u.args()? }
    };
    (f3rc, $u:ident, $v:ident, $index:ident, $first:ident, $count:ident) => {
        Instruction::$v { $index: u32::from($u.unit(1)).into(), $first: $u.unit(2), $count: $u.aa() }
    };
    (f45cc, $u:ident, $v:ident, $index:ident, $proto:ident, $args:ident) => {
        Instruction::$v {
            $index: u32::from($u.unit(1)).into(),
            $proto: u32::from($u.unit(3)).into(),
            $args: $u.args()?,
        }
    };
    (f4rcc, $u:ident, $v:ident, $index:ident, $proto:ident, $first:ident, $count:ident) => {
        Instruction::$v {
            $index: u32::from($u.unit(1)).into(),
            $proto: u32::from($u.unit(3)).into(),
            $first: $u.unit(2),
            $count: $u.aa(),
        }
    };
}

macro_rules! define_decoder {
    ($($variant:ident [$($shape:tt)*] [$($definition:tt)*] => $opname:ident $opcode:pat, $units:tt; [$($register:ident: $reg_type:ident $kind:ident $access:ident ($max:expr)),*]; $args:ident; [$($index:ident: $id_type:ident $pool:ident $at:literal $index_width:ident),*]; $format:ident($($slot:ident),*);)*) => {
        /// Decodes the instruction `frame` locates in `buf` onto `out`, where
        /// it is built in place.
        #[inline(always)]
        fn decode_onto(
            buf: &[u8],
            frame: &RawInstruction,
            out: &mut impl Extend<Instruction>,
        ) -> Result<()> {
            let units = Units {
                buf,
                at: frame.offset(),
                len: frame.units,
            };
            match u16::from(frame.opcode) {
                $(crate::instruction::opcodes::$opname => out.extend([decode_format!($format, units, $variant $(, $slot)*)]),)*
                _ => out.extend([units.raw()]),
            }
            Ok(())
        }
    };
}
instruction_catalogue!(define_decoder);
