// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::payload::split32;
use crate::error::{Result, invalid};
use crate::instruction::catalogue::instruction_catalogue;
use crate::instruction::{Instruction, RegList};

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
    (payload, $code:ident, $op:expr, $insn:ident) => { super::payload::encode($code, $insn)? };
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

macro_rules! define_encoder {
    ($($variant:ident [$($shape:tt)*] [$($definition:tt)*] => $opname:ident $opcode:pat, $units:tt; [$($register:ident: $reg_type:ident $kind:ident $access:ident ($max:expr)),*]; $args:ident; [$($index:ident: $id_type:ident $pool:ident $at:literal $index_width:ident),*]; $format:ident($($slot:ident),*);)*) => {
        fn encode(code: &mut Vec<u16>, insn: &Instruction) -> Result<()> {
            match insn {
                $(Instruction::$variant { $($slot,)* .. } => {
                    encode_format!($format, code, crate::instruction::opcodes::$opname, insn $(, $slot)*);
                })*
            }
            Ok(())
        }
    };
}
instruction_catalogue!(define_encoder);
