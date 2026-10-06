// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{
    BTreeMap, BoundaryTarget, BranchDestination, Instruction, Result, RewriteEntry, branch_offset,
    displacement, invalid, invert_condition, is_condition, is_payload, set_branch_offset, units,
};

pub(super) fn branch_destinations(
    expansions: &[RewriteEntry],
    targets: &BTreeMap<usize, BranchDestination>,
    original: &[u32],
) -> Result<Vec<Option<usize>>> {
    if targets.keys().any(|&index| {
        expansions
            .get(index)
            .and_then(|entry| entry.instruction.as_ref())
            .and_then(branch_offset)
            .is_none()
    }) {
        return Err(invalid(
            "instruction rewrite",
            "branch override must name a branch",
        ));
    }
    expansions
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            if let Some(destination) = targets.get(&index) {
                return match destination {
                    BranchDestination::Instruction(target) if *target <= expansions.len() => {
                        Ok(Some(*target))
                    }
                    BranchDestination::Instruction(_) => Err(invalid(
                        "instruction rewrite",
                        "branch target outside block",
                    )),
                    BranchDestination::External => Ok(None),
                };
            }
            entry
                .instruction
                .as_ref()
                .and_then(branch_offset)
                .map(|delta| target_index(original, index, delta))
                .transpose()
        })
        .collect()
}

pub(super) struct RewriteLayout {
    pub(super) starts: Vec<u32>,
    pub(super) bodies: Vec<u32>,
    pub(super) entries: Vec<usize>,
    pub(super) instruction_indices: Vec<usize>,
    pub(super) ends: Vec<usize>,
    pub(super) end: u32,
}

fn layout_entries(
    expansions: &[RewriteEntry],
    long_condition: &[bool],
    tail: &[Instruction],
) -> RewriteLayout {
    let mut starts = Vec::new();
    let mut bodies = Vec::new();
    let mut entries = Vec::new();
    let mut instruction_indices = Vec::new();
    let mut ends = Vec::new();
    let mut address = 0u32;
    let mut instruction_index = 0;
    for (i, expansion) in expansions.iter().enumerate() {
        if expansion.instruction.as_ref().is_some_and(is_payload) && !address.is_multiple_of(2) {
            address += 1;
            instruction_index += 1;
        }
        let prefix_address = address;
        let prefix_index = instruction_index;
        address += units(&expansion.before);
        instruction_index += expansion.before.len();
        bodies.push(address);
        entries.push(prefix_index);
        instruction_indices.push(instruction_index);
        match expansion.target {
            BoundaryTarget::Prefix => {
                starts.push(prefix_address);
            }
            BoundaryTarget::Body => {
                starts.push(address);
            }
        }
        address += expansion
            .instruction
            .as_ref()
            .map_or(0, Instruction::code_units)
            + units(&expansion.after);
        instruction_index += usize::from(expansion.instruction.is_some()) + expansion.after.len();
        if long_condition[i] {
            address += 3;
            instruction_index += 1;
        }
        ends.push(instruction_index);
    }
    address += units(tail);
    instruction_index += tail.len();
    starts.push(address);
    entries.push(instruction_index);
    instruction_indices.push(instruction_index);
    ends.push(instruction_index);
    RewriteLayout {
        starts,
        bodies,
        entries,
        instruction_indices,
        ends,
        end: address,
    }
}

fn target_index(original: &[u32], source: usize, delta: i32) -> Result<usize> {
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
}

pub(super) fn stabilized_layout(
    expansions: &mut [RewriteEntry],
    branch_targets: &[Option<usize>],
    long_condition: &mut [bool],
    tail: &[Instruction],
) -> Result<RewriteLayout> {
    use Instruction::{Goto, Goto16, Goto32};
    loop {
        let layout = layout_entries(expansions, long_condition, tail);
        let mut changed = false;
        for i in 0..expansions.len() {
            let Some(target_index) = branch_targets[i] else {
                continue;
            };
            let target = layout.starts[target_index];
            let delta = displacement(layout.bodies[i], target)?;
            let Some(instruction) = &mut expansions[i].instruction else {
                continue;
            };
            match instruction {
                Goto { .. } if i8::try_from(delta).is_err() => {
                    *instruction = if i16::try_from(delta).is_ok() {
                        Goto16 { offset: 0 }
                    } else {
                        Goto32 { offset: 0 }
                    };
                    changed = true;
                }
                Goto16 { .. } if i16::try_from(delta).is_err() => {
                    *instruction = Goto32 { offset: 0 };
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
            return Ok(layout);
        }
    }
}

pub(super) fn relocated_payloads(
    expansions: &[RewriteEntry],
    original: &[u32],
    layout: &RewriteLayout,
) -> Result<std::collections::HashMap<usize, Vec<i32>>> {
    use Instruction::{PackedSwitch, PackedSwitchPayload, SparseSwitch, SparseSwitchPayload};
    let mut payload_targets = std::collections::HashMap::new();
    for (i, entry) in expansions.iter().enumerate() {
        let Some(insn) = &entry.instruction else {
            continue;
        };
        let payload_offset = match insn {
            PackedSwitch { payload_offset, .. } | SparseSwitch { payload_offset, .. } => {
                *payload_offset
            }
            _ => continue,
        };
        let payload = target_index(original, i, payload_offset)?;
        let payload_insn = expansions
            .get(payload)
            .and_then(|entry| entry.instruction.as_ref())
            .ok_or_else(|| invalid("instruction rewrite", "missing switch payload"))?;
        let old_targets: Vec<i32> = match payload_insn {
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
                ));
            }
        };
        let targets = old_targets
            .into_iter()
            .map(|target| {
                displacement(
                    layout.bodies[i],
                    layout.starts[target_index(original, i, target)?],
                )
            })
            .collect::<Result<Vec<_>>>()?;
        if let Some(previous) = payload_targets.insert(payload, targets.clone())
            && previous != targets
        {
            return Err(invalid(
                "instruction rewrite",
                "shared switch payload needs different relocated targets",
            ));
        }
    }
    Ok(payload_targets)
}

pub(super) fn emit_entries(
    expansions: Vec<RewriteEntry>,
    tail: Vec<Instruction>,
    original: &[u32],
    layout: &RewriteLayout,
    branch_targets: &[Option<usize>],
    long_condition: &[bool],
    mut payload_targets: std::collections::HashMap<usize, Vec<i32>>,
) -> Result<Vec<Instruction>> {
    use Instruction::{
        FillArrayData, Goto32, Nop, PackedSwitch, PackedSwitchPayload, SparseSwitch,
        SparseSwitchPayload,
    };
    let mut instructions = Vec::new();
    let mut address = 0;
    for (i, mut expansion) in expansions.into_iter().enumerate() {
        if expansion.instruction.as_ref().is_some_and(is_payload) && address % 2 != 0 {
            instructions.push(Nop);
            address += 1;
        }
        if let Some(target_index) = branch_targets[i] {
            let destination = layout.starts[target_index];
            let instruction = expansion
                .instruction
                .as_mut()
                .expect("branch target belongs to a body instruction");
            if long_condition[i] {
                *instruction = invert_condition(instruction)?;
                set_branch_offset(instruction, 5)?;
                expansion.after.insert(
                    0,
                    Goto32 {
                        offset: displacement(layout.bodies[i] + 2, destination)?,
                    },
                );
            } else {
                set_branch_offset(instruction, displacement(layout.bodies[i], destination)?)?;
            }
        }
        match expansion.instruction.as_mut() {
            Some(
                PackedSwitch { payload_offset, .. }
                | SparseSwitch { payload_offset, .. }
                | FillArrayData { payload_offset, .. },
            ) => {
                let payload = layout.starts[target_index(original, i, *payload_offset)?];
                *payload_offset = displacement(layout.bodies[i], payload)?;
            }
            Some(PackedSwitchPayload(data)) => {
                if let Some(targets) = payload_targets.remove(&i) {
                    data.targets = targets;
                }
            }
            Some(SparseSwitchPayload(data)) => {
                if let Some(targets) = payload_targets.remove(&i) {
                    for ((_, target), relocated) in data.keys_and_targets.iter_mut().zip(targets) {
                        *target = relocated;
                    }
                }
            }
            _ => {}
        }
        address += units(&expansion.before)
            + expansion
                .instruction
                .as_ref()
                .map_or(0, Instruction::code_units)
            + units(&expansion.after);
        instructions.extend(expansion.before);
        instructions.extend(expansion.instruction);
        instructions.extend(expansion.after);
    }
    address += units(&tail);
    instructions.extend(tail);
    debug_assert_eq!(address, layout.end);
    Ok(instructions)
}
