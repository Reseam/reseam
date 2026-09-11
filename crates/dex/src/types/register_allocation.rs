// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Register-frame growth and legalization of instructions whose operands move
//! beyond their encoding. Narrow operands use dead registers; range invokes can
//! also use a shared argument area outside the caller's requested locals.

use super::code::CodeItem;
use super::code_rewrite::InstructionExpansion;
use super::instruction::Instruction;
use super::register_analysis::RegisterLiveness;
use super::register_operands::{map_operands, Access, RegisterKind};
use super::register_types::RegisterTypes;
use crate::error::{invalid, Result};
use crate::DexFile;

pub fn grow_registers(
    code: &mut CodeItem,
    additional: u16,
    incoming: &[String],
    dex: &DexFile,
) -> Result<Vec<usize>> {
    if additional == 0 {
        return Ok((0..=code.instructions.len()).collect());
    }
    let size = code
        .registers_size
        .checked_add(additional)
        .ok_or_else(|| invalid("register growth", "frame exceeds 65535 registers"))?;
    let base = code
        .registers_size
        .checked_sub(code.ins_size)
        .ok_or_else(|| invalid("register growth", "parameters exceed frame"))?;
    let liveness = RegisterLiveness::new(code);
    let incoming = incoming.iter().map(|ty| kind(ty)).collect::<Vec<_>>();
    if incoming
        .iter()
        .map(|kind| u32::from(kind.words()))
        .sum::<u32>()
        != u32::from(code.ins_size)
    {
        return Err(invalid(
            "register growth",
            "parameter width does not match ins_size",
        ));
    }
    let mut arguments = ArgumentArea {
        start: size,
        words: 0,
        incoming_words: code.ins_size,
    };
    let mut types = None;
    let mut expansions = Vec::with_capacity(code.instructions.len());
    for (index, instruction) in code.instructions.iter().enumerate() {
        let mut allocation = Allocation {
            code,
            index,
            base,
            additional,
            liveness: &liveness,
            used: vec![false; size as usize],
            arguments: &mut arguments,
        };
        instruction.visit_read_registers(|reg| {
            let shifted = allocation.shift(reg);
            allocation.used[shifted as usize] = true;
        });
        let mut expansion = InstructionExpansion::from(widen(instruction, &allocation));
        if instruction.is_invoke()
            || matches!(
                instruction,
                Instruction::FilledNewArray { .. } | Instruction::FilledNewArrayRange { .. }
            )
        {
            lower_invoke(&mut expansion, &mut allocation, dex)?;
        } else {
            if matches!(instruction, Instruction::RawInstruction { .. }) {
                return Err(invalid(
                    "register growth",
                    "cannot relocate an unknown instruction",
                ));
            }
            map_operands(&mut expansion.instruction, |operand| {
                let register = allocation.shift(operand.register);
                if operand.kind == RegisterKind::Wide
                    && allocation.shift(operand.register + 1) != register + 1
                {
                    return Err(invalid(
                        "register growth",
                        "a wide operand crosses the local/parameter boundary",
                    ));
                }
                if register <= operand.max {
                    return Ok(register);
                }
                let kind = if operand.kind == RegisterKind::Unknown {
                    if types.is_none() {
                        types = Some(RegisterTypes::new(code, &incoming)?);
                    }
                    types.as_ref().unwrap().kind(index, operand.register)?
                } else {
                    operand.kind
                };
                let scratch = allocation.scratch(kind.words(), operand.max)?;
                if operand.access != Access::Write {
                    expansion
                        .before
                        .push(move_register(scratch, register, kind));
                }
                if operand.access != Access::Read {
                    expansion.after.push(move_register(register, scratch, kind));
                }
                Ok(scratch)
            })?;
        }
        expansion.instruction = match expansion.instruction {
            Instruction::Move16 { dest, src } => move_register(dest, src, RegisterKind::Value),
            Instruction::MoveObject16 { dest, src } => {
                move_register(dest, src, RegisterKind::Object)
            }
            Instruction::MoveWide16 { dest, src } => move_register(dest, src, RegisterKind::Wide),
            instruction => instruction,
        };
        expansions.push(expansion);
    }
    let mut rewritten = code.clone();
    let mut indices = rewritten.rewrite_instructions(expansions)?;
    rewritten.registers_size = if arguments.words == 0 {
        size
    } else {
        size + arguments.words + code.ins_size
    };
    if arguments.words != 0 {
        // Keep body registers and requested locals at their planned positions.
        // The extra argument area moves the incoming window, so copy its values
        // down once at entry. Keep the actual incoming window above the argument
        // area, so a later growth never splits a staged wide value at its boundary.
        let mut entry = Vec::new();
        let mut destination = base + additional;
        for kind in incoming {
            entry.push(move_register(
                destination,
                destination + arguments.words + code.ins_size,
                kind,
            ));
            destination += kind.words();
        }
        let count = rewritten.instructions.len();
        rewritten.insert_instructions(0, &entry)?;
        let inserted = rewritten.instructions.len() - count;
        for index in &mut indices {
            *index += inserted;
        }
    }
    rewritten.outs_size = rewritten.compute_outs_size();
    *code = rewritten;
    Ok(indices)
}

struct Allocation<'a> {
    code: &'a CodeItem,
    index: usize,
    base: u16,
    additional: u16,
    liveness: &'a RegisterLiveness,
    used: Vec<bool>,
    arguments: &'a mut ArgumentArea,
}

/// Reused between invokes. This area never overlaps the method's relocated body
/// registers or the locals requested by the caller.
struct ArgumentArea {
    start: u16,
    words: u16,
    incoming_words: u16,
}

impl ArgumentArea {
    fn reserve(&mut self, words: u16) -> Result<u16> {
        self.start
            .checked_add(words)
            .and_then(|end| end.checked_add(self.incoming_words))
            .ok_or_else(|| invalid("register growth", "argument area exceeds 65535 registers"))?;
        self.words = self.words.max(words);
        Ok(self.start)
    }
}

impl Allocation<'_> {
    fn shift(&self, register: u16) -> u16 {
        if register >= self.base {
            register + self.additional
        } else {
            register
        }
    }

    fn scratch(&mut self, words: u16, max: u16) -> Result<u16> {
        self.dead_scratch(words, max).ok_or_else(|| {
            invalid(
                "register growth",
                format!(
                    "no dead {words}-word scratch span below v{max} at instruction {}",
                    self.index
                ),
            )
        })
    }

    fn dead_scratch(&mut self, words: u16, max: u16) -> Option<u16> {
        for original in 0..self.code.registers_size {
            let start = self.shift(original);
            if start > max {
                break;
            }
            let Some(end) = original.checked_add(words) else {
                continue;
            };
            if end > self.code.registers_size {
                break;
            }
            if (original..end).enumerate().all(|(word, reg)| {
                let shifted = self.shift(reg);
                shifted == start + word as u16
                    && !self.used[shifted as usize]
                    && !self.liveness.is_live(self.index, reg)
            }) {
                self.used[start as usize..start as usize + words as usize].fill(true);
                return Some(start);
            }
        }
        None
    }
}

fn kind(descriptor: &str) -> RegisterKind {
    match descriptor.as_bytes().first() {
        Some(b'J' | b'D') => RegisterKind::Wide,
        Some(b'L' | b'[') => RegisterKind::Object,
        _ => RegisterKind::Value,
    }
}

fn move_register(dest: u16, src: u16, kind: RegisterKind) -> Instruction {
    use Instruction::*;
    match kind {
        RegisterKind::Wide if dest <= 15 && src <= 15 => MoveWide {
            dest: dest as u8,
            src: src as u8,
        },
        RegisterKind::Wide if dest <= 255 => MoveWideFrom16 {
            dest: dest as u8,
            src,
        },
        RegisterKind::Wide => MoveWide16 { dest, src },
        RegisterKind::Object if dest <= 15 && src <= 15 => MoveObject {
            dest: dest as u8,
            src: src as u8,
        },
        RegisterKind::Object if dest <= 255 => MoveObjectFrom16 {
            dest: dest as u8,
            src,
        },
        RegisterKind::Object => MoveObject16 { dest, src },
        _ if dest <= 15 && src <= 15 => Move {
            dest: dest as u8,
            src: src as u8,
        },
        _ if dest <= 255 => MoveFrom16 {
            dest: dest as u8,
            src,
        },
        _ => Move16 { dest, src },
    }
}

// Select wider forms before asking the allocator for scratch registers.
fn widen(insn: &Instruction, allocation: &Allocation<'_>) -> Instruction {
    use Instruction::*;
    match *insn {
        Move { dest, src } => Move16 {
            dest: u16::from(dest),
            src: u16::from(src),
        },
        MoveFrom16 { dest, src } => Move16 {
            dest: u16::from(dest),
            src,
        },
        MoveWide { dest, src } => MoveWide16 {
            dest: u16::from(dest),
            src: u16::from(src),
        },
        MoveWideFrom16 { dest, src } => MoveWide16 {
            dest: u16::from(dest),
            src,
        },
        MoveObject { dest, src } => MoveObject16 {
            dest: u16::from(dest),
            src: u16::from(src),
        },
        MoveObjectFrom16 { dest, src } => MoveObject16 {
            dest: u16::from(dest),
            src,
        },
        Const4 { dest, value } if allocation.shift(u16::from(dest)) > 15 => Const16 {
            dest,
            value: i16::from(value),
        },
        AddInt2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            AddInt {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        SubInt2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            SubInt {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        MulInt2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            MulInt {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        DivInt2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            DivInt {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        RemInt2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            RemInt {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        AndInt2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            AndInt {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        OrInt2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            OrInt {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        XorInt2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            XorInt {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        ShlInt2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            ShlInt {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        ShrInt2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            ShrInt {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        UshrInt2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            UshrInt {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        AddLong2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            AddLong {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        SubLong2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            SubLong {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        MulLong2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            MulLong {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        DivLong2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            DivLong {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        RemLong2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            RemLong {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        AndLong2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            AndLong {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        OrLong2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            OrLong {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        XorLong2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            XorLong {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        ShlLong2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            ShlLong {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        ShrLong2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            ShrLong {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        UshrLong2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            UshrLong {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        AddFloat2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            AddFloat {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        SubFloat2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            SubFloat {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        MulFloat2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            MulFloat {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        DivFloat2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            DivFloat {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        RemFloat2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            RemFloat {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        AddDouble2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            AddDouble {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        SubDouble2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            SubDouble {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        MulDouble2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            MulDouble {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        DivDouble2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            DivDouble {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        RemDouble2Addr { dest_a, b }
            if allocation.shift(u16::from(dest_a)) > 15 || allocation.shift(u16::from(b)) > 15 =>
        {
            RemDouble {
                dest: dest_a,
                a: dest_a,
                b,
            }
        }
        _ => insn.clone(),
    }
}

fn lower_invoke(
    expansion: &mut InstructionExpansion,
    allocation: &mut Allocation<'_>,
    dex: &DexFile,
) -> Result<()> {
    use Instruction::*;
    let insn = &expansion.instruction;
    let registers: Vec<u16> = match insn {
        FilledNewArray { args, .. }
        | InvokeVirtual { args, .. }
        | InvokeSuper { args, .. }
        | InvokeDirect { args, .. }
        | InvokeStatic { args, .. }
        | InvokeInterface { args, .. }
        | InvokePolymorphic { args, .. }
        | InvokeCustom { args, .. } => args.iter().map(|reg| u16::from(*reg)).collect(),
        FilledNewArrayRange {
            first_reg, count, ..
        }
        | InvokeVirtualRange {
            first_reg, count, ..
        }
        | InvokeSuperRange {
            first_reg, count, ..
        }
        | InvokeDirectRange {
            first_reg, count, ..
        }
        | InvokeStaticRange {
            first_reg, count, ..
        }
        | InvokeInterfaceRange {
            first_reg, count, ..
        }
        | InvokePolymorphicRange {
            first_reg, count, ..
        }
        | InvokeCustomRange {
            first_reg, count, ..
        } => (*first_reg..*first_reg + u16::from(*count)).collect(),
        _ => unreachable!(),
    };
    let shifted: Vec<u16> = registers.iter().map(|reg| allocation.shift(*reg)).collect();
    let (arguments, prototype, receiver) = match insn {
        FilledNewArray { type_, .. } | FilledNewArrayRange { type_, .. } => {
            let descriptor = dex.type_descriptor(*type_);
            let element = descriptor.strip_prefix('[').ok_or_else(|| {
                invalid("register growth", "filled-new-array type is not an array")
            })?;
            (Some(vec![kind(element); registers.len()]), None, false)
        }
        InvokePolymorphic { proto, .. } | InvokePolymorphicRange { proto, .. } => {
            (None, Some(dex.proto(*proto)), true)
        }
        InvokeCustom { call_site, .. } | InvokeCustomRange { call_site, .. } => {
            let call_site = dex
                .call_sites
                .get(call_site.0 as usize)
                .ok_or_else(|| invalid("register growth", "invalid invoke-custom call site"))?;
            (None, Some(dex.proto(call_site.method_type)), false)
        }
        InvokeVirtual { method, .. }
        | InvokeVirtualRange { method, .. }
        | InvokeSuper { method, .. }
        | InvokeSuperRange { method, .. }
        | InvokeDirect { method, .. }
        | InvokeDirectRange { method, .. }
        | InvokeInterface { method, .. }
        | InvokeInterfaceRange { method, .. } => {
            (None, Some(dex.proto(dex.method_id(*method).proto)), true)
        }
        InvokeStatic { method, .. } | InvokeStaticRange { method, .. } => {
            (None, Some(dex.proto(dex.method_id(*method).proto)), false)
        }
        _ => unreachable!(),
    };
    let arguments = arguments.unwrap_or_else(|| {
        let mut arguments = Vec::new();
        if receiver {
            arguments.push(RegisterKind::Object);
        }
        for param in prototype.unwrap().parameters {
            arguments.push(kind(&dex.type_descriptor(param)));
        }
        arguments
    });
    if arguments
        .iter()
        .map(|kind| kind.words() as usize)
        .sum::<usize>()
        != registers.len()
    {
        return Err(invalid(
            "register growth",
            "invoke argument count does not match prototype",
        ));
    }
    let mut word = 0;
    for kind in &arguments {
        if *kind == RegisterKind::Wide && shifted[word + 1] != shifted[word] + 1 {
            return Err(invalid(
                "register growth",
                "wide invoke argument crosses the local/parameter boundary",
            ));
        }
        word += kind.words() as usize;
    }
    let compact = shifted.len() <= 5 && shifted.iter().all(|reg| *reg <= 15);
    let consecutive = shifted.windows(2).all(|pair| pair[1] == pair[0] + 1);
    let first = if compact || consecutive {
        shifted.first().copied().unwrap_or(0)
    } else {
        let words = shifted.len() as u16;
        let scratch = match allocation.dead_scratch(words, u16::MAX) {
            Some(register) => register,
            None => allocation.arguments.reserve(words)?,
        };
        let mut word = 0;
        for kind in arguments {
            expansion
                .before
                .push(move_register(scratch + word as u16, shifted[word], kind));
            word += kind.words() as usize;
        }
        scratch
    };
    let args = || shifted.iter().map(|reg| *reg as u8).collect();
    let count = u8::try_from(shifted.len())
        .map_err(|_| invalid("register growth", "invoke exceeds 255 words"))?;
    expansion.instruction = match *insn {
        FilledNewArray { type_, .. } | FilledNewArrayRange { type_, .. } => {
            if compact {
                FilledNewArray {
                    type_,
                    args: args(),
                }
            } else {
                FilledNewArrayRange {
                    type_,
                    first_reg: first,
                    count,
                }
            }
        }
        InvokeVirtual { method, .. } | InvokeVirtualRange { method, .. } => {
            if compact {
                InvokeVirtual {
                    method,
                    args: args(),
                }
            } else {
                InvokeVirtualRange {
                    method,
                    first_reg: first,
                    count,
                }
            }
        }
        InvokeSuper { method, .. } | InvokeSuperRange { method, .. } => {
            if compact {
                InvokeSuper {
                    method,
                    args: args(),
                }
            } else {
                InvokeSuperRange {
                    method,
                    first_reg: first,
                    count,
                }
            }
        }
        InvokeDirect { method, .. } | InvokeDirectRange { method, .. } => {
            if compact {
                InvokeDirect {
                    method,
                    args: args(),
                }
            } else {
                InvokeDirectRange {
                    method,
                    first_reg: first,
                    count,
                }
            }
        }
        InvokeStatic { method, .. } | InvokeStaticRange { method, .. } => {
            if compact {
                InvokeStatic {
                    method,
                    args: args(),
                }
            } else {
                InvokeStaticRange {
                    method,
                    first_reg: first,
                    count,
                }
            }
        }
        InvokeInterface { method, .. } | InvokeInterfaceRange { method, .. } => {
            if compact {
                InvokeInterface {
                    method,
                    args: args(),
                }
            } else {
                InvokeInterfaceRange {
                    method,
                    first_reg: first,
                    count,
                }
            }
        }
        InvokePolymorphic { method, proto, .. } | InvokePolymorphicRange { method, proto, .. } => {
            if compact {
                InvokePolymorphic {
                    method,
                    proto,
                    args: args(),
                }
            } else {
                InvokePolymorphicRange {
                    method,
                    proto,
                    first_reg: first,
                    count,
                }
            }
        }
        InvokeCustom { call_site, .. } | InvokeCustomRange { call_site, .. } => {
            if compact {
                InvokeCustom {
                    call_site,
                    args: args(),
                }
            } else {
                InvokeCustomRange {
                    call_site,
                    first_reg: first,
                    count,
                }
            }
        }
        _ => unreachable!(),
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CatchHandler, DexHeader, DexVersion, TryItem};

    fn dex() -> DexFile {
        fn header(version: DexVersion) -> DexHeader {
            DexHeader {
                version,
                checksum: 0,
                signature: [0; 20],
                file_size: 0,
                link_size: 0,
                link_off: 0,
                map_off: 0,
                string_ids_size: 0,
                string_ids_off: 0,
                type_ids_size: 0,
                type_ids_off: 0,
                proto_ids_size: 0,
                proto_ids_off: 0,
                field_ids_size: 0,
                field_ids_off: 0,
                method_ids_size: 0,
                method_ids_off: 0,
                class_defs_size: 0,
                class_defs_off: 0,
                data_size: 0,
                data_off: 0,
                container_size: 0,
                header_offset: 0,
            }
        }

        DexFile::new(header(DexVersion::V035))
    }

    fn code(ins_size: u16, instructions: Vec<Instruction>) -> CodeItem {
        CodeItem {
            registers_size: 16,
            ins_size,
            outs_size: 0,
            debug_info: None,
            instructions,
            tries: vec![],
            catch_handlers: vec![],
        }
    }

    #[test]
    fn lowers_wide_field_access_and_relocates_branches_and_handlers() {
        use Instruction::*;
        let mut dex = dex();
        let field = dex.intern_field("LExample;", "value", "J").unwrap();
        let mut code = code(
            1,
            vec![
                ConstWide16 { dest: 12, value: 7 },
                IfEqz { a: 15, offset: 3 },
                Nop,
                IgetWide {
                    dest: 12,
                    obj: 15,
                    field,
                },
                ReturnWide { src: 12 },
                MoveException { dest: 0 },
                ReturnWide { src: 12 },
            ],
        );
        code.tries.push(TryItem {
            start_addr: 5,
            insn_count: 2,
            handler_idx: 0,
        });
        code.catch_handlers.push(CatchHandler {
            typed_catches: vec![],
            catch_all_addr: Some(8),
        });
        let indices = grow_registers(&mut code, 3, &["LExample;".into()], &dex).unwrap();
        assert_eq!(code.registers_size, 19);
        assert_eq!(indices, [0, 1, 2, 3, 5, 6, 7, 8]);
        assert_eq!(code.instructions[3], MoveObjectFrom16 { dest: 0, src: 18 });
        assert_eq!(
            code.instructions[4],
            IgetWide {
                dest: 12,
                obj: 0,
                field
            }
        );
        assert_eq!(code.instructions[1], IfEqz { a: 18, offset: 3 });
        assert_eq!(code.tries[0].start_addr, 5);
        assert_eq!(code.tries[0].insn_count, 4);
        assert_eq!(code.catch_handlers[0].catch_all_addr, Some(10));
    }

    #[test]
    fn stages_reference_comparisons_with_object_moves() {
        use Instruction::*;
        let dex = dex();
        let mut code = code(
            2,
            vec![
                IfEq {
                    a: 14,
                    b: 15,
                    offset: 3,
                },
                ReturnVoid,
                ReturnVoid,
            ],
        );
        grow_registers(
            &mut code,
            3,
            &["Ljava/lang/Object;".into(), "Ljava/lang/Object;".into()],
            &dex,
        )
        .unwrap();
        assert_eq!(
            code.instructions,
            [
                MoveObjectFrom16 { dest: 0, src: 17 },
                MoveObjectFrom16 { dest: 1, src: 18 },
                IfEq {
                    a: 0,
                    b: 1,
                    offset: 3
                },
                ReturnVoid,
                ReturnVoid
            ]
        );
    }

    #[test]
    fn failure_preserves_code_instead_of_clobbering_a_live_register() {
        use Instruction::*;
        let mut dex = dex();
        let field = dex.intern_field("LExample;", "value", "I").unwrap();
        let method = dex
            .intern_method("LExample;", "consume", "(IIIIIIIIIIIIIIILExample;)V")
            .unwrap();
        let mut code = code(
            16,
            vec![
                Iput {
                    src: 14,
                    obj: 15,
                    field,
                },
                InvokeStaticRange {
                    method,
                    first_reg: 0,
                    count: 16,
                },
                ReturnVoid,
            ],
        );
        let original = code.clone();
        let mut incoming = vec!["I".to_string(); 15];
        incoming.push("LExample;".into());
        assert!(grow_registers(&mut code, 3, &incoming, &dex).is_err());
        assert_eq!(code, original);
    }
    #[test]
    fn grows_a_two_register_getter_by_thirty_two_locals() {
        use Instruction::*;
        let mut dex = dex();
        let field = dex
            .intern_field("LExample;", "items", "Ljava/util/List;")
            .unwrap();
        let mut code = code(
            1,
            vec![
                IgetObject {
                    dest: 0,
                    obj: 1,
                    field,
                },
                ReturnObject { src: 0 },
            ],
        );
        code.registers_size = 2;
        let indices = grow_registers(&mut code, 32, &["LExample;".into()], &dex).unwrap();
        assert_eq!(indices, [0, 2, 3]);
        assert_eq!(code.registers_size, 34);
        assert_eq!(
            code.instructions,
            [
                MoveObjectFrom16 { dest: 0, src: 33 },
                IgetObject {
                    dest: 0,
                    obj: 0,
                    field
                },
                ReturnObject { src: 0 }
            ]
        );
    }
}
