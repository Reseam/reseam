// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod allocation;
mod invoke;
mod widen;

use super::forms::move_register;
use allocation::{Allocation, ArgumentArea};

use super::liveness::RegisterLiveness;
use super::register_types::RegisterTypes;
use super::relocate::{InstructionExpansion, InstructionMap};
use crate::DexFile;
use crate::error::{Result, invalid};
use crate::instruction::operands::RegisterKind;
use crate::model::code::CodeItem;

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
    let expansions = code
        .instructions
        .iter()
        .enumerate()
        .map(|(index, instruction)| {
            let mut allocation = Allocation {
                code,
                index,
                base,
                additional,
                liveness: &liveness,
                used: vec![false; size as usize],
                arguments: &mut arguments,
            };
            allocation.relocate(instruction, protected, types.as_ref(), dex)
        })
        .collect::<Result<Vec<_>>>()?;
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
            Ok(RegisterKind::of(&dex.strings().get(descriptor)))
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
                .checked_add(operand.kind.map_or(1, RegisterKind::words))
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
            if operand.kind.is_none() && register > operand.max {
                queries.push((index, operand.register));
            }
        });
        result?;
    }
    Ok(queries)
}

#[cfg(test)]
mod tests;
