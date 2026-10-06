// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod invoke;
mod widen;

use invoke::lower_invoke;
use widen::widen;

use super::code::CodeItem;
use super::code_rewrite::{InstructionExpansion, InstructionMap};
use super::instruction::Instruction;
use super::register_analysis::RegisterLiveness;
use super::register_operands::{Access, RegisterKind, map_operands};
use super::register_types::RegisterTypes;
use crate::DexFile;
use crate::error::{Result, invalid};

/// `protected` names registers the caller holds across edits, which no operand
/// may stage through even where liveness says they are dead: a value one patch
/// block wrote for a block not yet emitted has no reader the analysis can see.
pub fn grow_registers(
    code: &mut CodeItem,
    additional: u16,
    incoming: &[crate::TypeIdx],
    protected: &[u16],
    dex: &DexFile,
) -> Result<InstructionMap> {
    if additional == 0 {
        return Ok(InstructionMap::identity(code.instructions.len()));
    }
    let (size, base) = grown_frame(code, additional)?;
    let liveness = RegisterLiveness::new(code);
    let incoming = incoming_kinds(incoming, code.ins_size, dex)?;
    let mut arguments = ArgumentArea {
        start: size,
        words: 0,
        incoming_words: code.ins_size,
    };
    let queries = displaced_queries(code, base, additional)?;
    let types = if queries.is_empty() {
        None
    } else {
        Some(RegisterTypes::new(code, &incoming, &queries)?)
    };
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
        for &register in protected {
            let shifted = allocation.shift(register);
            allocation.used[usize::from(shifted)] = true;
        }
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
            if matches!(instruction, Instruction::Raw { .. }) {
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
                    types
                        .as_ref()
                        .expect("displaced untyped operands were collected")
                        .kind(index, operand.register)?
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
    apply_expansions(code, additional, &incoming, &arguments, expansions)
}

fn grown_frame(code: &CodeItem, additional: u16) -> Result<(u16, u16)> {
    let size = code
        .registers_size
        .checked_add(additional)
        .ok_or_else(|| invalid("register growth", "frame exceeds 65535 registers"))?;
    let base = code
        .registers_size
        .checked_sub(code.ins_size)
        .ok_or_else(|| invalid("register growth", "parameters exceed frame"))?;
    Ok((size, base))
}

fn incoming_kinds(
    incoming: &[crate::TypeIdx],
    ins_size: u16,
    dex: &DexFile,
) -> Result<Vec<RegisterKind>> {
    let incoming = incoming
        .iter()
        .map(|&ty| {
            let descriptor = dex
                .types
                .try_get(ty.0 as usize)
                .ok_or_else(|| invalid("register growth", "incoming type outside pool"))?;
            if descriptor.0 as usize >= dex.strings.len() {
                return Err(invalid(
                    "register growth",
                    "incoming descriptor outside pool",
                ));
            }
            Ok(kind(&dex.string(descriptor)))
        })
        .collect::<Result<Vec<_>>>()?;
    if incoming
        .iter()
        .map(|kind| u32::from(kind.words()))
        .sum::<u32>()
        != u32::from(ins_size)
    {
        return Err(invalid(
            "register growth",
            "parameter width does not match ins_size",
        ));
    }
    Ok(incoming)
}

fn apply_expansions(
    code: &mut CodeItem,
    additional: u16,
    incoming: &[RegisterKind],
    arguments: &ArgumentArea,
    expansions: Vec<InstructionExpansion>,
) -> Result<InstructionMap> {
    let size = arguments.start;
    let base = code.registers_size - code.ins_size;
    let mut rewritten = code.clone();
    rewritten.debug_info = None;
    let mut indices =
        rewritten.rewrite_instructions(expansions, &std::collections::BTreeMap::new())?;
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
        for &kind in incoming {
            entry.push(move_register(
                destination,
                destination + arguments.words + code.ins_size,
                kind,
            ));
            destination += kind.words();
        }
        let insertion = rewritten.insert_instructions(0, &entry)?;
        for index in indices
            .starts
            .iter_mut()
            .chain(&mut indices.instructions)
            .chain(&mut indices.ends)
        {
            *index = insertion.instructions[*index];
        }
    }
    rewritten.outs_size = rewritten.compute_outs_size();
    *code = rewritten;
    Ok(indices)
}

fn displaced_queries(code: &CodeItem, base: u16, additional: u16) -> Result<Vec<(usize, u16)>> {
    let mut queries = Vec::new();
    for (index, instruction) in code.instructions.iter().enumerate() {
        let mut result = Ok(());
        instruction.visit_operands(|operand| {
            if operand
                .register
                .checked_add(operand.kind.words())
                .is_none_or(|end| end > code.registers_size)
            {
                result = Err(invalid("register growth", "operand outside frame"));
                return;
            }
            let register = if operand.register >= base {
                if let Some(register) = operand.register.checked_add(additional) {
                    register
                } else {
                    result = Err(invalid("register growth", "operand exceeds frame"));
                    return;
                }
            } else {
                operand.register
            };
            if operand.kind == RegisterKind::Unknown && register > operand.max {
                queries.push((index, operand.register));
            }
        });
        result?;
    }
    Ok(queries)
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
    use Instruction::{
        Move, Move16, MoveFrom16, MoveObject, MoveObject16, MoveObjectFrom16, MoveWide, MoveWide16,
        MoveWideFrom16,
    };
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

#[cfg(test)]
mod tests;
