// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;

use super::{BlockOperand, BlockRegister, BlockStep};
use crate::DexFile;
use crate::edit::forms::widest;
use crate::edit::invoke::invoke_kinds;
use crate::error::{Result, invalid};
use crate::instruction::Instruction;
use crate::instruction::operands::{Access, InstructionArguments, RegisterKind};

pub(super) struct Planned {
    /// The widest form, which reaches the most registers, with placeholder registers.
    pub(super) instruction: Instruction,
    pub(super) operands: Vec<BlockOperand>,
    pub(super) uses: Vec<Use>,
    /// The kinds of a call's arguments, which take no fixed registers.
    pub(super) arguments: Option<Vec<RegisterKind>>,
    /// The instruction a branch goes to, the block's length for its end.
    pub(super) target: Option<usize>,
}

#[derive(Clone, Copy)]
pub(super) struct Use {
    pub(super) access: Access,
    pub(super) max: u16,
}

/// The block's instructions in order, each branch pointing at the instruction its label marks.
pub(super) fn plan(steps: &[BlockStep], dex: &DexFile) -> Result<Vec<Planned>> {
    let mut labels = HashMap::new();
    let mut instructions = Vec::new();
    let mut targets = Vec::new();
    for step in steps {
        match step {
            BlockStep::Label(label) => {
                if labels.insert(*label, instructions.len()).is_some() {
                    return Err(invalid("code block", format!("label {label} placed twice")));
                }
            }
            BlockStep::Instruction {
                instruction,
                operands,
                target,
            } => {
                instructions.push(Planned::new(instruction, operands, dex)?);
                targets.push(*target);
            }
        }
    }
    for (planned, target) in instructions.iter_mut().zip(targets) {
        let Some(label) = target else { continue };
        if planned.instruction.branch_offset().is_none() {
            return Err(invalid("code block", "only a branch has a target"));
        }
        planned.target = Some(*labels.get(&label).ok_or_else(|| {
            invalid(
                "code block",
                format!("branch to label {label}, never placed"),
            )
        })?);
    }
    Ok(instructions)
}

impl Planned {
    fn new(instruction: &Instruction, operands: &[BlockOperand], dex: &DexFile) -> Result<Self> {
        let instruction = widest(instruction.clone());
        let arguments = if matches!(instruction.arguments(), InstructionArguments::None) {
            None
        } else {
            let words = usize::from(instruction.argument_words());
            Some(invoke_kinds(&instruction, words, dex)?)
        };
        let expected: Vec<(Option<RegisterKind>, Use)> = match &arguments {
            None => {
                let mut expected = Vec::new();
                instruction.visit_operands(|operand| {
                    let access = operand.access;
                    expected.push((
                        operand.kind,
                        Use {
                            access,
                            max: operand.max,
                        },
                    ));
                });
                expected
            }
            Some(kinds) => {
                let words: u16 = kinds.iter().map(|kind| kind.words()).sum();
                let max = if words <= 5 { 15 } else { u16::MAX };
                let access = Access::Read;
                kinds
                    .iter()
                    .map(|&kind| (Some(kind), Use { access, max }))
                    .collect()
            }
        };
        if expected.len() != operands.len()
            || expected.iter().zip(operands).any(|((kind, _), operand)| {
                kind.is_some_and(|kind| kind.words() != operand.kind.words())
            })
        {
            return Err(invalid(
                "code block",
                "operands do not match the instruction",
            ));
        }
        Ok(Self {
            instruction,
            operands: operands.to_vec(),
            uses: expected
                .into_iter()
                .map(|(_, operand_use)| operand_use)
                .collect(),
            arguments,
            target: None,
        })
    }

    pub(super) fn successors(&self, index: usize) -> impl Iterator<Item = usize> {
        self.instruction
            .falls_through()
            .then_some(index + 1)
            .into_iter()
            .chain(self.target)
    }

    pub(super) fn temporaries(
        &self,
        access: impl Fn(Access) -> bool + 'static,
    ) -> impl Iterator<Item = usize> + '_ {
        self.operands
            .iter()
            .zip(&self.uses)
            .filter(move |(_, operand)| access(operand.access))
            .filter_map(|(operand, _)| match operand.register {
                BlockRegister::Temporary(id) => Some(id as usize),
                BlockRegister::Fixed(_) => None,
            })
    }
}
