// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

mod layout;

use layout::{
    RewriteLayout, branch_destinations, emit_entries, relocated_payloads, stabilized_layout,
};
use std::collections::BTreeMap;

use super::code::CodeItem;
use super::instruction::Instruction;
use crate::error::{Result, invalid};

/// A branch authored in a partial instruction block can target another boundary
/// of that block, or retain its displacement into the surrounding method.
#[derive(Clone, Copy)]
pub enum BranchDestination {
    Instruction(usize),
    External,
}

/// Coordinates of each original instruction's expansion, including the final
/// boundary. Entry coordinates include staging; body coordinates identify the
/// instruction itself; end coordinates include its staged result.
#[derive(Debug, Clone)]
pub struct InstructionMap {
    pub(super) starts: Vec<usize>,
    pub(super) instructions: Vec<usize>,
    pub(super) ends: Vec<usize>,
}

impl InstructionMap {
    pub(super) fn identity(count: usize) -> Self {
        Self {
            starts: (0..=count).collect(),
            instructions: (0..=count).collect(),
            ends: (1..=count).chain([count]).collect(),
        }
    }

    pub fn starts(&self) -> &[usize] {
        &self.starts
    }
    pub fn instructions(&self) -> &[usize] {
        &self.instructions
    }
    pub fn ends(&self) -> &[usize] {
        &self.ends
    }
}

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

#[derive(Clone, Copy)]
pub(super) enum BoundaryTarget {
    Prefix,
    Body,
}

pub(super) struct RewriteEntry {
    pub before: Vec<Instruction>,
    pub instruction: Option<Instruction>,
    pub after: Vec<Instruction>,
    pub target: BoundaryTarget,
}

impl From<InstructionExpansion> for RewriteEntry {
    fn from(value: InstructionExpansion) -> Self {
        Self {
            before: value.before,
            instruction: Some(value.instruction),
            after: value.after,
            target: BoundaryTarget::Prefix,
        }
    }
}

impl CodeItem {
    pub(super) fn rewrite_entries(&self) -> Vec<RewriteEntry> {
        self.instructions
            .iter()
            .cloned()
            .map(InstructionExpansion::from)
            .map(RewriteEntry::from)
            .collect()
    }

    pub(super) fn has_payload_from(&self, index: usize) -> bool {
        self.instructions[index..].iter().any(is_payload)
    }

    /// Replaces each instruction with an expansion and returns the new index of
    /// each original boundary (including the end). Commits only after relocation succeeds.
    /// `targets` overrides encoded branch displacements with instruction identities
    /// during assembly; absent entries retain the original body's branch targets.
    pub fn rewrite_instructions(
        &mut self,
        expansions: Vec<InstructionExpansion>,
        targets: &BTreeMap<usize, BranchDestination>,
    ) -> Result<InstructionMap> {
        self.relocate_entries(
            expansions.into_iter().map(Into::into).collect(),
            Vec::new(),
            targets,
        )
    }

    /// Prepends each instruction's staging without copying instruction payloads.
    /// Consumes the code item; branch overrides and metadata use the original boundaries.
    /// Returns an error if prefix counts or relocation targets are invalid.
    pub fn prepend_instructions(
        mut self,
        prefixes: Vec<Vec<Instruction>>,
        targets: &BTreeMap<usize, BranchDestination>,
    ) -> Result<(Self, InstructionMap)> {
        if prefixes.len() != self.instructions.len() {
            return Err(invalid(
                "instruction rewrite",
                "prefix count does not match code",
            ));
        }
        let original = offsets(&self.instructions);
        let expansions = std::mem::take(&mut self.instructions)
            .into_iter()
            .zip(prefixes)
            .map(|(instruction, before)| RewriteEntry {
                before,
                instruction: Some(instruction),
                after: Vec::new(),
                target: BoundaryTarget::Prefix,
            })
            .collect();
        let mapping = self.relocate_with_offsets(expansions, Vec::new(), targets, &original)?;
        Ok((self, mapping))
    }

    pub(super) fn relocate_entries(
        &mut self,
        expansions: Vec<RewriteEntry>,
        tail: Vec<Instruction>,
        targets: &BTreeMap<usize, BranchDestination>,
    ) -> Result<InstructionMap> {
        if expansions.len() != self.instructions.len() {
            return Err(invalid(
                "instruction rewrite",
                "expansion count does not match code",
            ));
        }
        self.relocate_with_offsets(expansions, tail, targets, &offsets(&self.instructions))
    }

    fn relocate_with_offsets(
        &mut self,
        mut expansions: Vec<RewriteEntry>,
        tail: Vec<Instruction>,
        targets: &BTreeMap<usize, BranchDestination>,
        original: &[u32],
    ) -> Result<InstructionMap> {
        let branch_targets = branch_destinations(&expansions, targets, original)?;
        let mut long_condition = vec![false; expansions.len()];
        let layout =
            stabilized_layout(&mut expansions, &branch_targets, &mut long_condition, &tail)?;
        let relocate = |address| {
            original
                .binary_search(&address)
                .map(|index| layout.starts[index])
                .map_err(|_| {
                    invalid(
                        "instruction rewrite",
                        format!("metadata address {address} is not an instruction boundary (code ends at {})", original.last().expect("offsets include the end boundary")),
                    )
                })
        };
        let payload_targets = relocated_payloads(&expansions, original, &layout)?;
        let instructions = emit_entries(
            expansions,
            tail,
            original,
            &layout,
            &branch_targets,
            &long_condition,
            payload_targets,
        )?;
        let mut tries = self.tries.clone();
        for protected in &mut tries {
            let old_end = protected
                .start_addr
                .checked_add(u32::from(protected.insn_count))
                .ok_or_else(|| invalid("instruction rewrite", "try range overflow"))?;
            let end = relocate(old_end)?;
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
        let debug_info = self
            .debug_info
            .as_ref()
            .map(|metadata| {
                let mut metadata = metadata.clone();
                let debug = metadata.resolve_mut()?;
                *debug = relocate_debug(debug, &|address| {
                    relocate_debug_address(original, &layout, address)
                })?;
                Ok::<_, crate::DexError>(metadata)
            })
            .transpose()?;
        self.instructions = instructions;
        self.outs_size = self.compute_outs_size();
        self.tries = tries;
        self.catch_handlers = handlers;
        self.debug_info = debug_info;
        Ok(InstructionMap {
            starts: layout.entries,
            instructions: layout.instruction_indices,
            ends: layout.ends,
        })
    }
}

fn relocate_debug_address(original: &[u32], layout: &RewriteLayout, address: u32) -> Result<u32> {
    let index = original.partition_point(|&boundary| boundary <= address) - 1;
    let old_start = original[index];
    let new_start = if address == old_start {
        layout.starts[index]
    } else {
        layout.bodies.get(index).copied().unwrap_or(layout.end)
    };
    let within = address - old_start;
    let within = if let Some(&next) = layout.starts.get(index + 1) {
        within.min(next.saturating_sub(new_start))
    } else {
        within
    };
    new_start
        .checked_add(within)
        .ok_or_else(|| invalid("debug relocation", "address overflow"))
}

fn relocate_debug(
    debug: &super::debug::DebugInfo,
    relocate: &impl Fn(u32) -> Result<u32>,
) -> Result<super::debug::DebugInfo> {
    use super::debug::DebugBytecode;
    let mut debug = debug.clone();
    let mut old_address = 0u32;
    let mut new_address = relocate(0)?;
    for bytecode in &mut debug.bytecodes {
        let advance = match bytecode {
            DebugBytecode::AdvancePc { advance } => advance,
            DebugBytecode::SpecialAdvance { pc_advance, .. } => pc_advance,
            _ => continue,
        };
        old_address = old_address
            .checked_add(*advance)
            .ok_or_else(|| invalid("debug relocation", "address overflow"))?;
        let address = relocate(old_address)?;
        *advance = address
            .checked_sub(new_address)
            .ok_or_else(|| invalid("debug relocation", "address moved backwards"))?;
        new_address = address;
    }
    if relocate(0)? != 0 {
        debug.bytecodes.insert(
            0,
            DebugBytecode::AdvancePc {
                advance: relocate(0)?,
            },
        );
    }
    Ok(debug)
}

fn offsets(instructions: &[Instruction]) -> Vec<u32> {
    let mut offsets = Vec::with_capacity(instructions.len() + 1);
    let mut address = 0;
    for insn in instructions {
        offsets.push(address);
        address += insn.code_units();
    }
    offsets.push(address);
    offsets
}

fn units(instructions: &[Instruction]) -> u32 {
    instructions.iter().map(Instruction::code_units).sum()
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
/// The ordinary branch displacement, excluding payload references.
pub fn branch_offset(insn: &Instruction) -> Option<i32> {
    use Instruction::{
        Goto, Goto16, Goto32, IfEq, IfEqz, IfGe, IfGez, IfGt, IfGtz, IfLe, IfLez, IfLt, IfLtz,
        IfNe, IfNez,
    };
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
    use Instruction::{
        Goto, Goto16, Goto32, IfEq, IfEqz, IfGe, IfGez, IfGt, IfGtz, IfLe, IfLez, IfLt, IfLtz,
        IfNe, IfNez,
    };
    match insn {
        Goto { offset } => {
            *offset =
                i8::try_from(delta).map_err(|_| invalid("instruction rewrite", "goto overflow"))?;
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
    use Instruction::{
        IfEq, IfEqz, IfGe, IfGez, IfGt, IfGtz, IfLe, IfLez, IfLt, IfLtz, IfNe, IfNez,
    };
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
mod tests;
