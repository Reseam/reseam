// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;

use super::relocate::{BoundaryTarget, InstructionMap};
use crate::error::{Result, invalid};
use crate::instruction::Instruction;
use crate::model::code::CodeItem;

/// Where a branch to an instruction goes once code is inserted before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncomingBranches {
    /// Past the inserted code, to the instruction, as before the insertion.
    Skip,
    /// Into the inserted code, which then runs on every path to the instruction.
    Enter,
}

impl CodeItem {
    /// Edits a staged body. Changed instruction widths relocate existing branches
    /// and metadata once; any edit or relocation error leaves this item unchanged.
    /// Equal-width edits preserve metadata's original encoding.
    pub fn edit_instructions<T>(
        &mut self,
        edit: impl FnOnce(&mut [Instruction]) -> Result<T>,
    ) -> Result<T> {
        let mut instructions = self.instructions.clone();
        let result = edit(&mut instructions)?;
        if self
            .instructions
            .iter()
            .zip(&instructions)
            .all(|(old, new)| old.code_units() == new.code_units())
        {
            self.instructions = instructions;
            self.outs_size = self.compute_outs_size();
        } else {
            let entries = self
                .rewrite_entries()
                .into_iter()
                .zip(instructions)
                .map(|(mut entry, instruction)| {
                    entry.instruction = Some(instruction);
                    entry
                })
                .collect();
            self.relocate_entries(entries, Vec::new(), &BTreeMap::new())?;
        }
        Ok(result)
    }

    /// Replaces one instruction with a nonempty block. Incoming edges enter the
    /// block at its first instruction. Failed relocation leaves the body unchanged.
    pub fn replace_instructions(
        &mut self,
        index: usize,
        insns: &[Instruction],
    ) -> Result<InstructionMap> {
        self.ensure_index(index, self.instructions.len())?;
        let (last, before) = insns
            .split_last()
            .ok_or_else(|| invalid("instruction replacement", "replacement block is empty"))?;
        let delta = insns
            .iter()
            .map(|insn| i64::from(insn.code_units()))
            .sum::<i64>()
            - i64::from(self.instructions[index].code_units());
        let mut entries = self.rewrite_entries();
        entries[index].instruction = Some(last.clone());
        entries[index].before = before.to_vec();
        if delta % 2 != 0 && self.has_payload_from(index + 1) {
            entries[index].after.push(Instruction::Nop);
        }
        self.relocate_entries(entries, Vec::new(), &BTreeMap::new())
    }

    /// Inserts before `index`. Existing branches continue targeting the original
    /// instruction, skipping inserted code. Returns the relocated original boundaries,
    /// including the end. Failed relocation leaves the code unchanged.
    pub fn insert_instructions(
        &mut self,
        index: usize,
        insns: &[Instruction],
    ) -> Result<InstructionMap> {
        self.insert(index, insns, IncomingBranches::Skip)
    }

    pub(super) fn insert(
        &mut self,
        index: usize,
        insns: &[Instruction],
        branches: IncomingBranches,
    ) -> Result<InstructionMap> {
        self.ensure_index(index, self.instructions.len() + 1)?;
        let mut inserted = insns.to_vec();
        if insns.iter().map(Instruction::code_units).sum::<u32>() % 2 != 0
            && self.has_payload_from(index)
        {
            inserted.push(Instruction::Nop);
        }
        let mut entries = self.rewrite_entries();
        let tail = if let Some(entry) = entries.get_mut(index) {
            entry.before = inserted;
            entry.target = match branches {
                IncomingBranches::Skip => BoundaryTarget::Body,
                IncomingBranches::Enter => BoundaryTarget::Prefix,
            };
            Vec::new()
        } else {
            inserted
        };
        self.relocate_entries(entries, tail, &BTreeMap::new())
    }

    /// Removes exactly `count` original instructions, redirecting incoming edges
    /// to their successor. An invalid range or failed relocation leaves the body unchanged.
    pub fn remove_instructions(&mut self, index: usize, count: usize) -> Result<InstructionMap> {
        let end = index
            .checked_add(count)
            .filter(|end| *end <= self.instructions.len())
            .ok_or_else(|| invalid("instruction removal", "range outside body"))?;
        let mut entries = self.rewrite_entries();
        for entry in &mut entries[index..end] {
            entry.instruction = None;
        }
        if count != 0
            && self.instructions[index..end]
                .iter()
                .map(Instruction::code_units)
                .sum::<u32>()
                % 2
                != 0
            && self.has_payload_from(end)
        {
            entries[end - 1].after.push(Instruction::Nop);
        }
        self.relocate_entries(entries, Vec::new(), &BTreeMap::new())
    }

    pub fn set_instructions(&mut self, insns: Vec<Instruction>) {
        self.instructions = insns;
        self.tries.clear();
        self.catch_handlers.clear();
        self.debug_info = None;
        self.outs_size = self.compute_outs_size();
    }

    fn ensure_index(&self, index: usize, bound: usize) -> Result<()> {
        if index < bound {
            Ok(())
        } else {
            Err(invalid(
                "code item",
                format!(
                    "index {index} is out of bounds for instruction count {}",
                    self.instructions.len()
                ),
            ))
        }
    }
}

#[cfg(test)]
mod tests;
