// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;

use super::{
    BTreeMap, BoundaryTarget, BranchDestination, Instruction, Result, RewriteEntry, displacement,
    invalid, units,
};
use crate::instruction::branch::Addresses;

/// Where each entry of a rewritten body lands: addresses in code units and
/// positions in the new instruction list.
pub(super) struct RewriteLayout {
    /// Where control entering each original boundary lands, then the end.
    pub(super) starts: Vec<u32>,
    /// Where each entry's own instruction starts.
    pub(super) bodies: Vec<u32>,
    pub(super) entries: Vec<usize>,
    pub(super) instruction_indices: Vec<usize>,
    pub(super) ends: Vec<usize>,
    pub(super) end: u32,
}

/// A body being laid out again: each original instruction's expansion, the
/// entry each branch goes to, and the original addresses displacements and
/// metadata are read in.
pub(super) struct Rewrite<'a> {
    entries: Vec<RewriteEntry>,
    tail: Vec<Instruction>,
    branches: Vec<Option<usize>>,
    original: &'a Addresses,
}

impl<'a> Rewrite<'a> {
    /// `overrides` names the entries some branches go to instead of where
    /// their encoded displacement points in `original`.
    pub(super) fn new(
        entries: Vec<RewriteEntry>,
        tail: Vec<Instruction>,
        overrides: &BTreeMap<usize, BranchDestination>,
        original: &'a Addresses,
    ) -> Result<Self> {
        let branch = |index: usize| {
            entries
                .get(index)
                .and_then(|entry| entry.instruction.as_ref())
                .and_then(Instruction::branch_offset)
        };
        if overrides.keys().any(|&index| branch(index).is_none()) {
            return Err(invalid(
                "instruction rewrite",
                "branch override must name a branch",
            ));
        }
        let branches = (0..entries.len())
            .map(|index| match overrides.get(&index) {
                Some(BranchDestination::Instruction(target)) if *target <= entries.len() => {
                    Ok(Some(*target))
                }
                Some(BranchDestination::Instruction(_)) => Err(invalid(
                    "instruction rewrite",
                    "branch target outside block",
                )),
                Some(BranchDestination::External) => Ok(None),
                None => branch(index)
                    .map(|delta| target_index(original, index, delta))
                    .transpose(),
            })
            .collect::<Result<_>>()?;
        Ok(Self {
            entries,
            tail,
            branches,
            original,
        })
    }

    /// Widens each branch whose displacement outgrows its encoding until the
    /// layout settles: a goto takes a wider form, and a condition jumps over
    /// a goto/32 to its target.
    pub(super) fn stabilize(&mut self) -> Result<RewriteLayout> {
        loop {
            let layout = self.layout();
            let mut changed = false;
            for (i, entry) in self.entries.iter_mut().enumerate() {
                let (Some(target), Some(instruction)) = (self.branches[i], &mut entry.instruction)
                else {
                    continue;
                };
                let delta = displacement(layout.bodies[i], layout.starts[target])?;
                let wider = match instruction {
                    Instruction::Goto { .. } if i8::try_from(delta).is_err() => {
                        Some(if i16::try_from(delta).is_ok() {
                            Instruction::Goto16 { offset: 0 }
                        } else {
                            Instruction::Goto32 { offset: 0 }
                        })
                    }
                    Instruction::Goto16 { .. } if i16::try_from(delta).is_err() => {
                        Some(Instruction::Goto32 { offset: 0 })
                    }
                    _ => None,
                };
                if let Some(wider) = wider {
                    *instruction = wider;
                    changed = true;
                } else if instruction.is_condition()
                    && i16::try_from(delta).is_err()
                    && !entry.long_condition
                {
                    entry.long_condition = true;
                    changed = true;
                }
            }
            if !changed {
                return Ok(layout);
            }
        }
    }

    fn layout(&self) -> RewriteLayout {
        let mut layout = RewriteLayout {
            starts: Vec::new(),
            bodies: Vec::new(),
            entries: Vec::new(),
            instruction_indices: Vec::new(),
            ends: Vec::new(),
            end: 0,
        };
        let mut address = 0u32;
        let mut index = 0;
        for entry in &self.entries {
            if entry
                .instruction
                .as_ref()
                .is_some_and(Instruction::is_payload)
                && !address.is_multiple_of(2)
            {
                address += 1;
                index += 1;
            }
            let (prefix_address, prefix_index) = (address, index);
            address += units(&entry.before);
            index += entry.before.len();
            layout.bodies.push(address);
            layout.entries.push(prefix_index);
            layout.instruction_indices.push(index);
            layout.starts.push(match entry.target {
                BoundaryTarget::Prefix => prefix_address,
                BoundaryTarget::Body => address,
            });
            address += entry
                .instruction
                .as_ref()
                .map_or(0, Instruction::code_units)
                + units(&entry.after);
            index += usize::from(entry.instruction.is_some()) + entry.after.len();
            if entry.long_condition {
                address += 3;
                index += 1;
            }
            layout.ends.push(index);
        }
        address += units(&self.tail);
        index += self.tail.len();
        layout.starts.push(address);
        layout.entries.push(index);
        layout.instruction_indices.push(index);
        layout.ends.push(index);
        layout.end = address;
        layout
    }

    /// The new instruction list, every displacement relative to `layout`.
    pub(super) fn emit(self, layout: &RewriteLayout) -> Result<Vec<Instruction>> {
        let mut switch_targets = self.switch_targets(layout)?;
        let mut instructions = Vec::new();
        let mut address = 0;
        for (i, mut entry) in self.entries.into_iter().enumerate() {
            if entry
                .instruction
                .as_ref()
                .is_some_and(Instruction::is_payload)
                && address % 2 != 0
            {
                instructions.push(Instruction::Nop);
                address += 1;
            }
            if let Some(target) = self.branches[i] {
                let destination = layout.starts[target];
                let instruction = entry
                    .instruction
                    .as_mut()
                    .expect("branch target belongs to a body instruction");
                if entry.long_condition {
                    *instruction = instruction.inverted_condition()?;
                    instruction.set_branch_offset(5)?;
                    entry.after.insert(
                        0,
                        Instruction::Goto32 {
                            offset: displacement(layout.bodies[i] + 2, destination)?,
                        },
                    );
                } else {
                    instruction.set_branch_offset(displacement(layout.bodies[i], destination)?)?;
                }
            }
            if let Some(instruction) = entry.instruction.as_mut() {
                if let Some(payload_offset) = instruction.payload_offset_mut() {
                    let payload = layout.starts[target_index(self.original, i, *payload_offset)?];
                    *payload_offset = displacement(layout.bodies[i], payload)?;
                } else if let Some(targets) = switch_targets.remove(&i) {
                    instruction.set_switch_targets(targets);
                }
            }
            address += units(&entry.before)
                + entry
                    .instruction
                    .as_ref()
                    .map_or(0, Instruction::code_units)
                + units(&entry.after);
            instructions.extend(entry.before);
            instructions.extend(entry.instruction);
            instructions.extend(entry.after);
        }
        address += units(&self.tail);
        instructions.extend(self.tail);
        debug_assert_eq!(address, layout.end);
        Ok(instructions)
    }

    /// The relocated targets of each switch payload, keyed by its entry. A
    /// payload two switches share must relocate the same way for both.
    fn switch_targets(&self, layout: &RewriteLayout) -> Result<HashMap<usize, Vec<i32>>> {
        let mut relocated = HashMap::new();
        for (i, entry) in self.entries.iter().enumerate() {
            let Some(switch) = entry.instruction.as_ref().filter(|insn| insn.is_switch()) else {
                continue;
            };
            let offset = switch.payload_offset().expect("a switch reads a payload");
            let payload = target_index(self.original, i, offset)?;
            let targets = self
                .entries
                .get(payload)
                .and_then(|entry| entry.instruction.as_ref())
                .and_then(Instruction::switch_targets)
                .ok_or_else(|| {
                    invalid(
                        "instruction rewrite",
                        "switch target is not a switch payload",
                    )
                })?
                .into_iter()
                .map(|target| {
                    let target = layout.starts[target_index(self.original, i, target)?];
                    displacement(layout.bodies[i], target)
                })
                .collect::<Result<Vec<_>>>()?;
            if let Some(previous) = relocated.insert(payload, targets.clone())
                && previous != targets
            {
                return Err(invalid(
                    "instruction rewrite",
                    "shared switch payload needs different relocated targets",
                ));
            }
        }
        Ok(relocated)
    }
}

fn target_index(original: &Addresses, source: usize, delta: i32) -> Result<usize> {
    original.target(source, delta).ok_or_else(|| {
        invalid(
            "instruction rewrite",
            "target is not an instruction boundary",
        )
    })
}
