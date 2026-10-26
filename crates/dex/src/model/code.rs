// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::TypeIdx;
use super::debug::DebugInfo;
use crate::error::Result;
use crate::error::invalid;
use crate::instruction::Instruction;

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

    /// Outgoing argument words the code needs: what the instructions require
    /// after any edits, never less than what the method was compiled with.
    pub fn compute_outs_size(&self) -> u16 {
        self.instructions
            .iter()
            .map(Instruction::argument_words)
            .max()
            .unwrap_or(0)
            .max(self.outs_size)
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
