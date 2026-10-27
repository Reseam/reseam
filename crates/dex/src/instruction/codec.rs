// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! One decoder and one encoder per instruction format; the catalogue names
//! each instruction's format and which of its fields fills each slot.

use super::catalogue::instruction_catalogue;
use super::opcodes::{FILL_ARRAY_DATA_PAYLOAD, PACKED_SWITCH_PAYLOAD, SPARSE_SWITCH_PAYLOAD};
use super::stream::{RawInstruction, instruction_stream};
use super::{FillArrayPayloadData, Instruction, PackedSwitchData, RegList, SparseSwitchData};
use crate::error::{Result, invalid, malformed, require_len};
use crate::read::{i32_at, u16_at, u32_at};

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

/// Encodes instructions as code units. An operand that exceeds its encoding
/// fails. Pool indices must already belong to the owning DEX.
pub fn encode_instructions(instructions: &[Instruction]) -> Result<Vec<u16>> {
    let mut code = Vec::with_capacity(
        instructions
            .iter()
            .map(|instruction| instruction.code_units() as usize)
            .sum(),
    );
    for instruction in instructions {
        encode(&mut code, instruction)?;
    }
    Ok(code)
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
        let at = self.at;
        let size = || usize::from(self.unit(1));
        Ok(match self.unit(0) {
            0 => Instruction::Nop,
            PACKED_SWITCH_PAYLOAD => {
                let size = size();
                require_len(self.buf, at, 8 + size * 4, "packed-switch payload")?;
                Instruction::PackedSwitchPayload(Box::new(PackedSwitchData {
                    first_key: i32_at(self.buf, at + 4),
                    targets: (0..size)
                        .map(|i| i32_at(self.buf, at + 8 + i * 4))
                        .collect(),
                }))
            }
            SPARSE_SWITCH_PAYLOAD => {
                let size = size();
                require_len(self.buf, at, 4 + size * 8, "sparse-switch payload")?;
                let pair = |i| {
                    (
                        i32_at(self.buf, at + 4 + i * 4),
                        i32_at(self.buf, at + 4 + size * 4 + i * 4),
                    )
                };
                Instruction::SparseSwitchPayload(Box::new(SparseSwitchData {
                    keys_and_targets: (0..size).map(pair).collect(),
                }))
            }
            FILL_ARRAY_DATA_PAYLOAD => {
                let bytes = (self.word(2) as usize).checked_mul(size()).ok_or_else(|| {
                    malformed("fill-array-data payload", at, "payload size overflowed")
                })?;
                require_len(self.buf, at, 8 + bytes, "fill-array-data payload")?;
                Instruction::FillArrayDataPayload(Box::new(FillArrayPayloadData {
                    element_width: self.unit(1),
                    data: self.buf[at + 8..at + 8 + bytes].to_vec(),
                }))
            }
            _ => self.raw(),
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

/// The register nibbles of a 12x-style first unit.
fn nibbles(op: u16, a: u8, b: u8) -> Result<u16> {
    if let Some(register) = [a, b].into_iter().find(|&register| register > 15) {
        return Err(invalid(
            "instruction",
            format!("register v{register} exceeds the nibble range of its operand"),
        ));
    }
    Ok(op | u16::from(a) << 8 | u16::from(b) << 12)
}

fn op_aa(op: u16, aa: u8) -> u16 {
    op | u16::from(aa) << 8
}

fn index16(index: u32) -> Result<u16> {
    u16::try_from(index)
        .map_err(|_| invalid("instruction", "pool index exceeds its 16-bit operand"))
}

fn literal4(value: i8) -> Result<u8> {
    if !(-8..=7).contains(&value) {
        return Err(invalid(
            "instruction",
            format!("literal {value} does not fit const/4's signed nibble"),
        ));
    }
    Ok(value as u8 & 0xf)
}

fn split32(value: u32) -> [u16; 2] {
    [value as u16, (value >> 16) as u16]
}

/// The units of a 35c instruction, which 45cc extends: the argument count
/// and G, the index, then C, D, E and F.
fn compact(code: &mut Vec<u16>, op: u16, index: u32, args: &RegList) -> Result<()> {
    if let Some(register) = args.iter().find(|&&register| register > 15) {
        return Err(invalid(
            "instruction",
            format!("register v{register} exceeds the nibble range of a compact argument"),
        ));
    }
    let mut registers = [0u16; 5];
    for (slot, &register) in registers.iter_mut().zip(args.iter()) {
        *slot = u16::from(register);
    }
    let [first, second, third, fourth, fifth] = registers;
    code.extend([
        op | fifth << 8 | (args.len() as u16) << 12,
        index16(index)?,
        first | second << 4 | third << 8 | fourth << 12,
    ]);
    Ok(())
}

macro_rules! encode_format {
    (nop, $code:ident, $op:expr, $insn:ident) => { $code.push($op) };
    (payload, $code:ident, $op:expr, $insn:ident) => { encode_payload($code, $insn)? };
    (raw, $code:ident, $op:expr, $insn:ident) => {
        if let Instruction::Raw { code_units } = $insn {
            $code.extend_from_slice(code_units);
        }
    };
    (f10x, $code:ident, $op:expr, $insn:ident) => { $code.push($op) };
    (f12x, $code:ident, $op:expr, $insn:ident, $a:ident, $b:ident) => { $code.push(nibbles($op, *$a, *$b)?) };
    (f11n, $code:ident, $op:expr, $insn:ident, $a:ident, $b:ident) => { $code.push(nibbles($op, *$a, literal4(*$b)?)?) };
    (f11x, $code:ident, $op:expr, $insn:ident, $a:ident) => { $code.push(op_aa($op, *$a)) };
    (f10t, $code:ident, $op:expr, $insn:ident, $a:ident) => { $code.push(op_aa($op, *$a as u8)) };
    (f20t, $code:ident, $op:expr, $insn:ident, $a:ident) => { $code.extend([$op, *$a as u16]) };
    (f30t, $code:ident, $op:expr, $insn:ident, $a:ident) => {{
        $code.push($op);
        $code.extend(split32(*$a as u32));
    }};
    (f22x, $code:ident, $op:expr, $insn:ident, $a:ident, $b:ident) => { $code.extend([op_aa($op, *$a), *$b]) };
    (f32x, $code:ident, $op:expr, $insn:ident, $a:ident, $b:ident) => { $code.extend([$op, *$a, *$b]) };
    (f21t, $($rest:tt)*) => { encode_format!(f21s, $($rest)*) };
    (f21h, $($rest:tt)*) => { encode_format!(f21s, $($rest)*) };
    (f21s, $code:ident, $op:expr, $insn:ident, $a:ident, $b:ident) => { $code.extend([op_aa($op, *$a), *$b as u16]) };
    (f21c, $code:ident, $op:expr, $insn:ident, $a:ident, $b:ident) => { $code.extend([op_aa($op, *$a), index16($b.0)?]) };
    (f23x, $code:ident, $op:expr, $insn:ident, $a:ident, $b:ident, $c:ident) => {
        $code.extend([op_aa($op, *$a), u16::from(*$b) | u16::from(*$c) << 8])
    };
    (f22b, $code:ident, $op:expr, $insn:ident, $a:ident, $b:ident, $c:ident) => {
        $code.extend([op_aa($op, *$a), u16::from(*$b) | u16::from(*$c as u8) << 8])
    };
    (f22t, $($rest:tt)*) => { encode_format!(f22s, $($rest)*) };
    (f22s, $code:ident, $op:expr, $insn:ident, $a:ident, $b:ident, $c:ident) => {
        $code.extend([nibbles($op, *$a, *$b)?, *$c as u16])
    };
    (f22c, $code:ident, $op:expr, $insn:ident, $a:ident, $b:ident, $c:ident) => {
        $code.extend([nibbles($op, *$a, *$b)?, index16($c.0)?])
    };
    (f31t, $($rest:tt)*) => { encode_format!(f31i, $($rest)*) };
    (f31i, $code:ident, $op:expr, $insn:ident, $a:ident, $b:ident) => {{
        $code.push(op_aa($op, *$a));
        $code.extend(split32(*$b as u32));
    }};
    (f31c, $code:ident, $op:expr, $insn:ident, $a:ident, $b:ident) => {{
        $code.push(op_aa($op, *$a));
        $code.extend(split32($b.0));
    }};
    (f51l, $code:ident, $op:expr, $insn:ident, $a:ident, $b:ident) => {{
        $code.push(op_aa($op, *$a));
        $code.extend(split32(*$b as u32));
        $code.extend(split32((*$b >> 32) as u32));
    }};
    (f35c, $code:ident, $op:expr, $insn:ident, $index:ident, $args:ident) => { compact($code, $op, $index.0, $args)? };
    (f3rc, $code:ident, $op:expr, $insn:ident, $index:ident, $first:ident, $count:ident) => {
        $code.extend([op_aa($op, *$count), index16($index.0)?, *$first])
    };
    (f45cc, $code:ident, $op:expr, $insn:ident, $index:ident, $proto:ident, $args:ident) => {{
        compact($code, $op, $index.0, $args)?;
        $code.push(index16($proto.0)?);
    }};
    (f4rcc, $code:ident, $op:expr, $insn:ident, $index:ident, $proto:ident, $first:ident, $count:ident) => {
        $code.extend([op_aa($op, *$count), index16($index.0)?, *$first, index16($proto.0)?])
    };
}

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

macro_rules! define_codec {
    ($($variant:ident [$($shape:tt)*] [$($definition:tt)*] => $opname:ident $opcode:pat, $units:tt; [$($register:ident: $reg_type:ident $kind:ident $access:ident ($max:expr)),*]; $args:ident; [$($index:ident: $id_type:ident $pool:ident $at:literal $index_width:ident),*]; $format:ident($($slot:ident),*);)*) => {
        /// Decodes the instruction `frame` locates in `buf` onto `out`, where
        /// it is built in place.
        #[inline(always)]
        pub(crate) fn decode_onto(
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
                $(super::opcodes::$opname => out.extend([decode_format!($format, units, $variant $(, $slot)*)]),)*
                _ => out.extend([units.raw()]),
            }
            Ok(())
        }

        fn encode(code: &mut Vec<u16>, insn: &Instruction) -> Result<()> {
            match insn {
                $(Instruction::$variant { $($slot,)* .. } => {
                    encode_format!($format, code, super::opcodes::$opname, insn $(, $slot)*);
                })*
            }
            Ok(())
        }

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
instruction_catalogue!(define_codec);

fn encode_payload(code: &mut Vec<u16>, insn: &Instruction) -> Result<()> {
    match insn {
        Instruction::PackedSwitchPayload(payload) => {
            let size = u16::try_from(payload.targets.len())
                .map_err(|_| invalid("packed-switch payload", "too many targets"))?;
            code.extend([PACKED_SWITCH_PAYLOAD, size]);
            code.extend(split32(payload.first_key as u32));
            code.extend(
                payload
                    .targets
                    .iter()
                    .flat_map(|&target| split32(target as u32)),
            );
        }
        Instruction::SparseSwitchPayload(payload) => {
            let pairs = &payload.keys_and_targets;
            let size = u16::try_from(pairs.len())
                .map_err(|_| invalid("sparse-switch payload", "too many keys"))?;
            code.extend([SPARSE_SWITCH_PAYLOAD, size]);
            code.extend(pairs.iter().flat_map(|&(key, _)| split32(key as u32)));
            code.extend(pairs.iter().flat_map(|&(_, target)| split32(target as u32)));
        }
        Instruction::FillArrayDataPayload(payload) => {
            let width = usize::from(payload.element_width);
            if width == 0 || !payload.data.len().is_multiple_of(width) {
                return Err(invalid(
                    "fill-array payload",
                    "data must contain complete elements with nonzero width",
                ));
            }
            let count = u32::try_from(payload.data.len() / width)
                .map_err(|_| invalid("fill-array payload", "element count exceeds u32"))?;
            code.extend([FILL_ARRAY_DATA_PAYLOAD, payload.element_width]);
            code.extend(split32(count));
            let (pairs, tail) = payload.data.as_chunks::<2>();
            code.extend(pairs.iter().map(|&pair| u16::from_le_bytes(pair)));
            code.extend(tail.iter().map(|&last| u16::from(last)));
        }
        _ => return Err(invalid("instruction encoding", "expected a payload")),
    }
    Ok(())
}
