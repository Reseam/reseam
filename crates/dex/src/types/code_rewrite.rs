// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Instruction expansion with relocation in code units. Original instruction
//! boundaries remain the identity of branch, payload, and exception targets.

use super::code::CodeItem;
use super::instruction::Instruction;
use crate::error::{invalid, Result};

pub struct InstructionExpansion {
    pub before: Vec<Instruction>,
    pub instruction: Instruction,
    pub after: Vec<Instruction>,
}

impl From<Instruction> for InstructionExpansion {
    fn from(instruction: Instruction) -> Self {
        Self {
            before: Vec::new(),
            instruction,
            after: Vec::new(),
        }
    }
}

impl CodeItem {
    /// Replaces each instruction with an expansion and returns the new index of
    /// each original boundary (including the end). Commits only after relocation succeeds.
    pub fn rewrite_instructions(
        &mut self,
        mut expansions: Vec<InstructionExpansion>,
    ) -> Result<Vec<usize>> {
        use Instruction::*;
        if expansions.len() != self.instructions.len() {
            return Err(invalid(
                "instruction rewrite",
                "expansion count does not match code",
            ));
        }
        let original = offsets(&self.instructions);
        let target_index = |source: usize, delta: i32| -> Result<usize> {
            let target = i64::from(original[source]) + i64::from(delta);
            u32::try_from(target)
                .ok()
                .and_then(|addr| original.binary_search(&addr).ok())
                .ok_or_else(|| {
                    invalid(
                        "instruction rewrite",
                        "target is not an instruction boundary",
                    )
                })
        };
        let mut long_condition = vec![false; expansions.len()];
        let (starts, bodies, indices, end) = loop {
            let mut starts = Vec::new();
            let mut bodies = Vec::new();
            let mut indices = Vec::new();
            let mut address = 0u32;
            let mut instruction_index = 0;
            for (i, expansion) in expansions.iter().enumerate() {
                if is_payload(&expansion.instruction) && address % 2 != 0 {
                    address += 1;
                    instruction_index += 1;
                }
                starts.push(address);
                indices.push(instruction_index);
                address += units(&expansion.before);
                bodies.push(address);
                address += expansion.instruction.code_units() as u32 + units(&expansion.after);
                instruction_index += expansion.before.len() + 1 + expansion.after.len();
                if long_condition[i] {
                    address += 3;
                    instruction_index += 1;
                }
            }
            starts.push(address);
            indices.push(instruction_index);
            let mut changed = false;
            for i in 0..expansions.len() {
                let Some(delta) = branch_offset(&self.instructions[i]) else {
                    continue;
                };
                let target = starts[target_index(i, delta)?];
                let delta = displacement(bodies[i], target)?;
                match &mut expansions[i].instruction {
                    Goto { .. } if i8::try_from(delta).is_err() => {
                        expansions[i].instruction = if i16::try_from(delta).is_ok() {
                            Goto16 { offset: 0 }
                        } else {
                            Goto32 { offset: 0 }
                        };
                        changed = true;
                    }
                    Goto16 { .. } if i16::try_from(delta).is_err() => {
                        expansions[i].instruction = Goto32 { offset: 0 };
                        changed = true;
                    }
                    insn if is_condition(insn)
                        && i16::try_from(delta).is_err()
                        && !long_condition[i] =>
                    {
                        long_condition[i] = true;
                        changed = true;
                    }
                    _ => {}
                }
            }
            if !changed {
                break (starts, bodies, indices, address);
            }
        };
        let relocate = |address: u32| -> Result<u32> {
            original
                .binary_search(&address)
                .ok()
                .map(|i| starts[i])
                .ok_or_else(|| {
                    invalid(
                        "instruction rewrite",
                        "metadata target is not an instruction boundary",
                    )
                })
        };
        let mut payload_targets = std::collections::HashMap::new();
        for (i, insn) in self.instructions.iter().enumerate() {
            let payload_offset = match insn {
                PackedSwitch { payload_offset, .. } | SparseSwitch { payload_offset, .. } => {
                    *payload_offset
                }
                _ => continue,
            };
            let payload = target_index(i, payload_offset)?;
            let old_targets: Vec<i32> = match &self.instructions[payload] {
                PackedSwitchPayload(data) => data.targets.clone(),
                SparseSwitchPayload(data) => data
                    .keys_and_targets
                    .iter()
                    .map(|(_, target)| *target)
                    .collect(),
                _ => {
                    return Err(invalid(
                        "instruction rewrite",
                        "switch target is not a switch payload",
                    ))
                }
            };
            let targets = old_targets
                .into_iter()
                .map(|target| displacement(bodies[i], starts[target_index(i, target)?]))
                .collect::<Result<Vec<_>>>()?;
            if let Some(previous) = payload_targets.insert(payload, targets.clone()) {
                if previous != targets {
                    return Err(invalid(
                        "instruction rewrite",
                        "shared switch payload needs different relocated targets",
                    ));
                }
            }
        }
        let mut instructions = Vec::new();
        let mut address = 0;
        for (i, mut expansion) in expansions.into_iter().enumerate() {
            if is_payload(&expansion.instruction) && address % 2 != 0 {
                instructions.push(Nop);
                address += 1;
            }
            if let Some(delta) = branch_offset(&self.instructions[i]) {
                let destination = starts[target_index(i, delta)?];
                if long_condition[i] {
                    expansion.instruction = invert_condition(&expansion.instruction)?;
                    set_branch_offset(&mut expansion.instruction, 5)?;
                    expansion.after.insert(
                        0,
                        Goto32 {
                            offset: displacement(bodies[i] + 2, destination)?,
                        },
                    );
                } else {
                    set_branch_offset(
                        &mut expansion.instruction,
                        displacement(bodies[i], destination)?,
                    )?;
                }
            }
            match &mut expansion.instruction {
                PackedSwitch { payload_offset, .. }
                | SparseSwitch { payload_offset, .. }
                | FillArrayData { payload_offset, .. } => {
                    let payload = starts[target_index(i, *payload_offset)?];
                    *payload_offset = displacement(bodies[i], payload)?;
                }
                PackedSwitchPayload(data) => {
                    if let Some(targets) = payload_targets.remove(&i) {
                        data.targets = targets;
                    }
                }
                SparseSwitchPayload(data) => {
                    if let Some(targets) = payload_targets.remove(&i) {
                        for ((_, target), relocated) in
                            data.keys_and_targets.iter_mut().zip(targets)
                        {
                            *target = relocated;
                        }
                    }
                }
                _ => {}
            }
            address += units(&expansion.before)
                + expansion.instruction.code_units() as u32
                + units(&expansion.after);
            instructions.extend(expansion.before);
            instructions.push(expansion.instruction);
            instructions.extend(expansion.after);
        }
        debug_assert_eq!(address, end);
        let mut tries = self.tries.clone();
        for protected in &mut tries {
            let end = relocate(protected.start_addr + u32::from(protected.insn_count))?;
            protected.start_addr = relocate(protected.start_addr)?;
            protected.insn_count = u16::try_from(end - protected.start_addr).map_err(|_| {
                invalid(
                    "instruction rewrite",
                    "expanded try range exceeds 65535 code units",
                )
            })?;
        }
        let mut handlers = self.catch_handlers.clone();
        for handler in &mut handlers {
            for catch in &mut handler.typed_catches {
                catch.addr = relocate(catch.addr)?;
            }
            handler.catch_all_addr = handler.catch_all_addr.map(relocate).transpose()?;
        }
        self.instructions = instructions;
        self.outs_size = self.compute_outs_size();
        self.tries = tries;
        self.catch_handlers = handlers;
        self.debug_info = None;
        Ok(indices)
    }
}

fn offsets(instructions: &[Instruction]) -> Vec<u32> {
    let mut offsets = Vec::with_capacity(instructions.len() + 1);
    let mut address = 0;
    for insn in instructions {
        offsets.push(address);
        address += insn.code_units() as u32;
    }
    offsets.push(address);
    offsets
}

fn units(instructions: &[Instruction]) -> u32 {
    instructions
        .iter()
        .map(|insn| insn.code_units() as u32)
        .sum()
}
fn displacement(source: u32, target: u32) -> Result<i32> {
    i32::try_from(i64::from(target) - i64::from(source))
        .map_err(|_| invalid("instruction rewrite", "branch displacement exceeds i32"))
}
fn is_payload(insn: &Instruction) -> bool {
    matches!(
        insn,
        Instruction::PackedSwitchPayload(_)
            | Instruction::SparseSwitchPayload(_)
            | Instruction::FillArrayDataPayload(_)
    )
}
fn branch_offset(insn: &Instruction) -> Option<i32> {
    use Instruction::*;
    match insn {
        Goto { offset } => Some(i32::from(*offset)),
        Goto16 { offset }
        | IfEq { offset, .. }
        | IfNe { offset, .. }
        | IfLt { offset, .. }
        | IfGe { offset, .. }
        | IfGt { offset, .. }
        | IfLe { offset, .. }
        | IfEqz { offset, .. }
        | IfNez { offset, .. }
        | IfLtz { offset, .. }
        | IfGez { offset, .. }
        | IfGtz { offset, .. }
        | IfLez { offset, .. } => Some(i32::from(*offset)),
        Goto32 { offset } => Some(*offset),
        _ => None,
    }
}
fn is_condition(insn: &Instruction) -> bool {
    branch_offset(insn).is_some()
        && !matches!(
            insn,
            Instruction::Goto { .. } | Instruction::Goto16 { .. } | Instruction::Goto32 { .. }
        )
}
fn set_branch_offset(insn: &mut Instruction, delta: i32) -> Result<()> {
    use Instruction::*;
    match insn {
        Goto { offset } => {
            *offset =
                i8::try_from(delta).map_err(|_| invalid("instruction rewrite", "goto overflow"))?
        }
        Goto16 { offset }
        | IfEq { offset, .. }
        | IfNe { offset, .. }
        | IfLt { offset, .. }
        | IfGe { offset, .. }
        | IfGt { offset, .. }
        | IfLe { offset, .. }
        | IfEqz { offset, .. }
        | IfNez { offset, .. }
        | IfLtz { offset, .. }
        | IfGez { offset, .. }
        | IfGtz { offset, .. }
        | IfLez { offset, .. } => {
            *offset = i16::try_from(delta)
                .map_err(|_| invalid("instruction rewrite", "conditional overflow"))?;
        }
        Goto32 { offset } => *offset = delta,
        _ => return Err(invalid("instruction rewrite", "expected branch")),
    }
    Ok(())
}
fn invert_condition(insn: &Instruction) -> Result<Instruction> {
    use Instruction::*;
    Ok(match *insn {
        IfEq { a, b, offset } => IfNe { a, b, offset },
        IfNe { a, b, offset } => IfEq { a, b, offset },
        IfLt { a, b, offset } => IfGe { a, b, offset },
        IfGe { a, b, offset } => IfLt { a, b, offset },
        IfGt { a, b, offset } => IfLe { a, b, offset },
        IfLe { a, b, offset } => IfGt { a, b, offset },
        IfEqz { a, offset } => IfNez { a, offset },
        IfNez { a, offset } => IfEqz { a, offset },
        IfLtz { a, offset } => IfGez { a, offset },
        IfGez { a, offset } => IfLtz { a, offset },
        IfGtz { a, offset } => IfLez { a, offset },
        IfLez { a, offset } => IfGtz { a, offset },
        _ => return Err(invalid("instruction rewrite", "expected condition")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(instructions: Vec<Instruction>) -> CodeItem {
        CodeItem {
            registers_size: 2,
            ins_size: 0,
            outs_size: 0,
            debug_info: None,
            instructions,
            tries: vec![],
            catch_handlers: vec![],
        }
    }

    #[test]
    fn expanded_switch_targets_run_the_prefix_and_payloads_stay_aligned() {
        use Instruction::*;
        let mut code = code(vec![
            PackedSwitch {
                test: 0,
                payload_offset: 4,
            },
            ReturnVoid,
            PackedSwitchPayload(Box::new(crate::PackedSwitchData {
                first_key: 0,
                targets: vec![3],
            })),
        ]);
        let mut expansions: Vec<InstructionExpansion> =
            code.instructions.iter().cloned().map(Into::into).collect();
        expansions[1].before.push(Const4 { dest: 1, value: 0 });
        let indices = code.rewrite_instructions(expansions).unwrap();
        assert_eq!(indices, [0, 1, 4, 5]);
        assert_eq!(
            code.instructions,
            [
                PackedSwitch {
                    test: 0,
                    payload_offset: 6
                },
                Const4 { dest: 1, value: 0 },
                ReturnVoid,
                Nop,
                PackedSwitchPayload(Box::new(crate::PackedSwitchData {
                    first_key: 0,
                    targets: vec![3]
                }))
            ]
        );
    }

    #[test]
    fn widens_forward_conditions_and_backward_gotos_after_expansion() {
        use Instruction::*;
        let mut conditional = code(vec![IfEqz { a: 0, offset: 3 }, Nop, ReturnVoid]);
        let mut expansions: Vec<InstructionExpansion> = conditional
            .instructions
            .iter()
            .cloned()
            .map(Into::into)
            .collect();
        expansions[1].before = vec![Nop; 33_000];
        let indices = conditional.rewrite_instructions(expansions).unwrap();
        assert_eq!(conditional.instructions[0], IfNez { a: 0, offset: 5 });
        assert_eq!(conditional.instructions[1], Goto32 { offset: 33_004 });
        assert_eq!(indices[2], 33_003);

        let mut backward = code(vec![Nop, Goto { offset: -1 }]);
        let mut expansions: Vec<InstructionExpansion> = backward
            .instructions
            .iter()
            .cloned()
            .map(Into::into)
            .collect();
        expansions[0].before = vec![Nop; 300];
        let indices = backward.rewrite_instructions(expansions).unwrap();
        assert_eq!(backward.instructions[indices[1]], Goto16 { offset: -301 });
    }
}
