// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;

use super::TypeIdx;
use super::code_rewrite::InstructionMap;
use super::debug::DebugInfo;
use super::instruction::Instruction;
use crate::error::Result;
use crate::error::invalid;

#[derive(Debug, Clone, PartialEq)]
pub struct CodeItem {
    pub(crate) registers_size: u16,
    pub(crate) ins_size: u16,
    pub(crate) outs_size: u16,
    pub(crate) debug_info: Option<super::metadata::Metadata<DebugInfo>>,
    pub(crate) instructions: Vec<Instruction>,
    pub(crate) tries: Vec<TryItem>,
    pub(crate) catch_handlers: Vec<CatchHandler>,
}

impl CodeItem {
    /// Authors a body with a register frame. Encoding widths and pool references
    /// are checked by serialization; the incoming words must fit the frame.
    pub fn new(
        registers_size: u16,
        ins_size: u16,
        outs_size: u16,
        instructions: Vec<Instruction>,
    ) -> Result<Self> {
        if ins_size > registers_size {
            return Err(invalid(
                "code item",
                "incoming words exceed the register frame",
            ));
        }
        Ok(Self {
            registers_size,
            ins_size,
            outs_size,
            instructions,
            debug_info: None,
            tries: Vec::new(),
            catch_handlers: Vec::new(),
        })
    }

    pub fn instructions(&self) -> &[Instruction] {
        &self.instructions
    }
    /// Consumes the code item, transferring its instruction storage without copying it.
    pub fn into_instructions(self) -> Vec<Instruction> {
        self.instructions
    }

    pub fn registers_size(&self) -> u16 {
        self.registers_size
    }
    pub fn ins_size(&self) -> u16 {
        self.ins_size
    }
    pub fn outs_size(&self) -> u16 {
        self.outs_size
    }
    pub fn debug_info(&self) -> Option<&super::metadata::Metadata<DebugInfo>> {
        self.debug_info.as_ref()
    }
    pub fn tries(&self) -> &[TryItem] {
        &self.tries
    }
    pub fn catch_handlers(&self) -> &[CatchHandler] {
        &self.catch_handlers
    }

    /// Replaces authored debug positions. Addresses use the current body coordinates.
    pub fn set_debug_info(&mut self, info: Option<super::metadata::Metadata<DebugInfo>>) {
        self.debug_info = info;
    }

    /// Replaces exception metadata in the current body coordinates. Missing
    /// handlers are an error and leave the previous metadata unchanged.
    pub fn set_exception_handlers(
        &mut self,
        tries: Vec<TryItem>,
        handlers: Vec<CatchHandler>,
    ) -> Result<()> {
        if tries.iter().any(|item| item.handler_idx >= handlers.len()) {
            return Err(invalid(
                "code item",
                "try refers to a missing catch handler",
            ));
        }
        self.tries = tries;
        self.catch_handlers = handlers;
        Ok(())
    }

    /// Changes the declared register frame without shifting operands. For added
    /// locals that move incoming registers, use `grow_registers` instead.
    pub fn set_register_frame(
        &mut self,
        registers_size: u16,
        ins_size: u16,
        outs_size: u16,
    ) -> Result<()> {
        if ins_size > registers_size {
            return Err(invalid(
                "code item",
                "incoming words exceed the register frame",
            ));
        }
        self.registers_size = registers_size;
        self.ins_size = ins_size;
        self.outs_size = outs_size;
        Ok(())
    }

    pub fn ensure_outs_size(&mut self, words: u16) {
        self.outs_size = self.outs_size.max(words);
    }

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

    /// Replaces the body and declared frame together, dropping old coordinate
    /// metadata. An invalid frame leaves the previous body unchanged.
    pub fn replace_body(
        &mut self,
        registers_size: u16,
        outs_size: u16,
        instructions: Vec<Instruction>,
    ) -> Result<()> {
        if self.ins_size > registers_size {
            return Err(invalid(
                "code item",
                "incoming words exceed the register frame",
            ));
        }
        self.set_instructions(instructions);
        self.registers_size = registers_size;
        self.outs_size = outs_size;
        Ok(())
    }

    /// Outgoing argument words the code needs: what the instructions require
    /// after any edits, never less than what the method was compiled with.
    pub fn compute_outs_size(&self) -> u16 {
        self.instructions
            .iter()
            .map(Instruction::outgoing_arg_count)
            .max()
            .unwrap_or(0)
            .max(self.outs_size)
    }

    /// Replaces one instruction and returns the relocated original boundaries, including the end.
    pub fn replace_instruction(
        &mut self,
        index: usize,
        insn: Instruction,
    ) -> Result<InstructionMap> {
        self.replace_instructions(index, &[insn])
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

    pub fn insert_instruction(&mut self, index: usize, insn: Instruction) -> Result<()> {
        self.insert_instructions(index, &[insn]).map(drop)
    }

    /// Inserts before `index`. Existing branches continue targeting the original
    /// instruction, skipping inserted code. Returns the relocated original boundaries,
    /// including the end. Failed relocation leaves the code unchanged.
    pub fn insert_instructions(
        &mut self,
        index: usize,
        insns: &[Instruction],
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
            entry.target = super::code_rewrite::BoundaryTarget::Body;
            Vec::new()
        } else {
            inserted
        };
        self.relocate_entries(entries, tail, &BTreeMap::new())
    }

    /// Removes one instruction and returns the relocated original boundaries, including the end.
    pub fn remove_instruction(&mut self, index: usize) -> Result<InstructionMap> {
        self.remove_instructions(index, 1)
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

    pub fn return_early(&mut self) {
        self.set_instructions(vec![Instruction::ReturnVoid]);
        self.registers_size = self.ins_size;
        self.outs_size = 0;
        self.debug_info = None;
    }

    pub fn return_early_int(&mut self, value: i32) {
        self.set_instructions(vec![
            Instruction::Const { dest: 0, value },
            Instruction::Return { src: 0 },
        ]);
        self.registers_size = self.ins_size.max(1);
        self.outs_size = 0;
        self.debug_info = None;
    }

    pub fn return_early_object(&mut self, value: i32) {
        self.set_instructions(vec![
            Instruction::Const { dest: 0, value },
            Instruction::ReturnObject { src: 0 },
        ]);
        self.registers_size = self.ins_size.max(1);
        self.outs_size = 0;
        self.debug_info = None;
    }

    pub fn return_early_wide(&mut self, value: i64) {
        self.set_instructions(vec![
            Instruction::ConstWide { dest: 0, value },
            Instruction::ReturnWide { src: 0 },
        ]);
        self.registers_size = self.ins_size.max(2);
        self.outs_size = 0;
        self.debug_info = None;
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

#[derive(Debug, Clone, PartialEq)]
pub struct TryItem {
    pub start_addr: u32,
    pub insn_count: u16,
    pub handler_idx: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatchHandler {
    pub typed_catches: Vec<TypedCatch>,
    pub catch_all_addr: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedCatch {
    pub exception_type: TypeIdx,
    pub addr: u32,
}

#[cfg(test)]
mod tests;
