// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;

use super::allocate::Plan;
use super::planned::Planned;
use super::program::Program;
use crate::edit::forms::{move_register, narrowed};
use crate::edit::invoke::{Invocation, InvokeForm, invoke_encoding};
use crate::edit::relocate::{BranchDestination, InstructionExpansion, assemble};
use crate::error::Result;
use crate::instruction::Instruction;
use crate::instruction::operands::{Access, map_operands};

/// The block's instructions in the registers `plan` chose, laid out with every branch in reach.
pub(super) fn emit(program: &Program, plan: &Plan) -> Result<Vec<Instruction>> {
    let expansions = program
        .instructions
        .iter()
        .enumerate()
        .map(|(index, planned)| expansion(planned, plan, index))
        .collect::<Result<Vec<_>>>()?;
    let targets: BTreeMap<usize, BranchDestination> = program
        .instructions
        .iter()
        .enumerate()
        .filter_map(|(index, planned)| {
            planned
                .target
                .map(|target| (index, BranchDestination::Instruction(target)))
        })
        .collect();
    assemble(expansions, &targets)
}

fn expansion(planned: &Planned, plan: &Plan, index: usize) -> Result<InstructionExpansion> {
    let registers: Vec<u16> = planned
        .operands
        .iter()
        .map(|operand| plan.register(operand.register))
        .collect();
    if let Some(kinds) = &planned.arguments {
        let words: Vec<u16> = registers
            .iter()
            .zip(kinds)
            .flat_map(|(&register, kind)| (0..kind.words()).map(move |word| register + word))
            .collect();
        let (moves, encoding) =
            Invocation::new(InvokeForm::Compact, &words, kinds)?.lower(plan.spans[index])?;
        return Ok(InstructionExpansion {
            before: moves
                .iter()
                .map(|argument| move_register(argument.dest, argument.src, argument.kind))
                .collect(),
            instruction: invoke_encoding(&planned.instruction, &words, encoding)?,
            after: Vec::new(),
        });
    }
    let mut instruction = planned.instruction.clone();
    let (mut before, mut after) = (Vec::new(), Vec::new());
    let mut position = 0;
    map_operands(&mut instruction, |operand| {
        let register = registers[position];
        let staging = plan.staging[index][position];
        let kind = planned.operands[position].kind;
        position += 1;
        let Some(scratch) = staging else {
            return Ok(register);
        };
        if operand.access != Access::Write {
            before.push(move_register(scratch, register, kind));
        }
        if operand.access != Access::Read {
            after.push(move_register(register, scratch, kind));
        }
        Ok(scratch)
    })?;
    Ok(InstructionExpansion {
        before,
        instruction: narrowed(instruction),
        after,
    })
}
