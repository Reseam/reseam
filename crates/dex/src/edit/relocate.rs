// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod layout;
mod metadata;

use layout::{Rewrite, RewriteLayout};
use metadata::Metadata;
use std::collections::BTreeMap;

use crate::error::{Result, invalid};
use crate::instruction::Instruction;
use crate::instruction::branch::Addresses;
use crate::model::code::CodeItem;

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
    /// The condition reaches its target through a goto/32 it jumps over.
    pub long_condition: bool,
}

impl From<InstructionExpansion> for RewriteEntry {
    fn from(value: InstructionExpansion) -> Self {
        Self {
            before: value.before,
            instruction: Some(value.instruction),
            after: value.after,
            target: BoundaryTarget::Prefix,
            long_condition: false,
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
        self.instructions[index..]
            .iter()
            .any(Instruction::is_payload)
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
        let original = Addresses::of(&self.instructions);
        let expansions = std::mem::take(&mut self.instructions)
            .into_iter()
            .zip(prefixes)
            .map(|(instruction, before)| RewriteEntry {
                before,
                instruction: Some(instruction),
                after: Vec::new(),
                target: BoundaryTarget::Prefix,
                long_condition: false,
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
        self.relocate_with_offsets(
            expansions,
            tail,
            targets,
            &Addresses::of(&self.instructions),
        )
    }

    fn relocate_with_offsets(
        &mut self,
        expansions: Vec<RewriteEntry>,
        tail: Vec<Instruction>,
        targets: &BTreeMap<usize, BranchDestination>,
        original: &Addresses,
    ) -> Result<InstructionMap> {
        let mut rewrite = Rewrite::new(expansions, tail, targets, original)?;
        let layout = rewrite.stabilize()?;
        let metadata = Metadata::relocated(self, original, &layout)?;
        let instructions = rewrite.emit(&layout)?;
        self.instructions = instructions;
        self.outs_size = self.compute_outs_size();
        self.tries = metadata.tries;
        self.catch_handlers = metadata.handlers;
        self.debug_info = metadata.debug_info;
        Ok(InstructionMap {
            starts: layout.entries,
            instructions: layout.instruction_indices,
            ends: layout.ends,
        })
    }
}

/// Lays out a standalone block whose branches name the expansion they go to, widening each
/// branch its layout puts out of reach.
pub(super) fn assemble(
    expansions: Vec<InstructionExpansion>,
    targets: &BTreeMap<usize, BranchDestination>,
) -> Result<Vec<Instruction>> {
    let original = Addresses::of(
        &expansions
            .iter()
            .map(|expansion| expansion.instruction.clone())
            .collect::<Vec<_>>(),
    );
    let entries = expansions.into_iter().map(RewriteEntry::from).collect();
    let mut rewrite = Rewrite::new(entries, Vec::new(), targets, &original)?;
    let layout = rewrite.stabilize()?;
    rewrite.emit(&layout)
}

fn units(instructions: &[Instruction]) -> u32 {
    instructions.iter().map(Instruction::code_units).sum()
}
fn displacement(source: u32, target: u32) -> Result<i32> {
    i32::try_from(i64::from(target) - i64::from(source))
        .map_err(|_| invalid("instruction rewrite", "branch displacement exceeds i32"))
}

#[cfg(test)]
mod tests;
