// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::invoke::lower_invoke;
use super::widen::widen;
use crate::DexFile;
use crate::edit::forms::{move_register, narrowed_move};
use crate::edit::liveness::RegisterLiveness;
use crate::edit::register_types::RegisterTypes;
use crate::edit::relocate::InstructionExpansion;
use crate::error::{Result, invalid};
use crate::instruction::Instruction;
use crate::instruction::operands::{Access, RegisterKind, map_operands};
use crate::model::code::CodeItem;

/// Register choices for one instruction of a body whose frame grows by
/// `additional` locals, inserted at `base` below the incoming registers.
pub(super) struct Allocation<'a> {
    pub(super) code: &'a CodeItem,
    pub(super) index: usize,
    pub(super) base: u16,
    pub(super) additional: u16,
    pub(super) liveness: &'a RegisterLiveness,
    pub(super) used: Vec<bool>,
    pub(super) arguments: &'a mut ArgumentArea,
}

pub(super) struct ArgumentArea {
    pub(super) start: u16,
    pub(super) words: u16,
    pub(super) incoming_words: u16,
}

impl ArgumentArea {
    pub(super) fn reserve(&mut self, words: u16) -> Result<u16> {
        self.start
            .checked_add(words)
            .and_then(|end| end.checked_add(self.incoming_words))
            .ok_or_else(|| invalid("register growth", "argument area exceeds 65535 registers"))?;
        self.words = self.words.max(words);
        Ok(self.start)
    }
}

impl Allocation<'_> {
    /// The instruction's expansion in the grown frame: its operands shifted,
    /// staged through low scratch registers where an encoding cannot reach them.
    pub(super) fn relocate(
        &mut self,
        instruction: &Instruction,
        protected: &[u16],
        types: Option<&RegisterTypes>,
        dex: &DexFile,
    ) -> Result<InstructionExpansion> {
        for &register in protected {
            let shifted = self.shift(register);
            self.used[usize::from(shifted)] = true;
        }
        instruction.visit_read_registers(|register| {
            let shifted = self.shift(register);
            self.used[usize::from(shifted)] = true;
        });
        let mut expansion = InstructionExpansion::from(widen(instruction, self));
        if instruction.is_invoke()
            || matches!(
                instruction,
                Instruction::FilledNewArray { .. } | Instruction::FilledNewArrayRange { .. }
            )
        {
            lower_invoke(&mut expansion, self, dex)?;
        } else if matches!(instruction, Instruction::Raw { .. }) {
            return Err(invalid(
                "register growth",
                "cannot relocate an unknown instruction",
            ));
        } else {
            self.stage_operands(&mut expansion, types)?;
        }
        expansion.instruction = narrowed_move(expansion.instruction);
        Ok(expansion)
    }

    /// Shifts each operand, staging one an encoding cannot reach through a
    /// dead scratch register it can, copied in before and out after.
    fn stage_operands(
        &mut self,
        expansion: &mut InstructionExpansion,
        types: Option<&RegisterTypes>,
    ) -> Result<()> {
        let index = self.index;
        map_operands(&mut expansion.instruction, |operand| {
            let register = self.shift(operand.register);
            if operand.kind == Some(RegisterKind::Wide)
                && self.shift(operand.register + 1) != register + 1
            {
                return Err(invalid(
                    "register growth",
                    "a wide operand crosses the local/parameter boundary",
                ));
            }
            if register <= operand.max {
                return Ok(register);
            }
            let kind = match operand.kind {
                Some(kind) => kind,
                None => types
                    .expect("displaced untyped operands were collected")
                    .kind(index, operand.register)?,
            };
            let scratch = self.scratch(kind.words(), operand.max)?;
            if operand.access != Access::Write {
                expansion
                    .before
                    .push(move_register(scratch, register, kind));
            }
            if operand.access != Access::Read {
                expansion.after.push(move_register(register, scratch, kind));
            }
            Ok(scratch)
        })
    }

    pub(super) fn shift(&self, register: u16) -> u16 {
        if register >= self.base {
            register + self.additional
        } else {
            register
        }
    }

    fn scratch(&mut self, words: u16, max: u16) -> Result<u16> {
        self.dead_scratch(words, max).ok_or_else(|| {
            invalid(
                "register growth",
                format!(
                    "no dead {words}-word scratch span below v{max} at instruction {}",
                    self.index
                ),
            )
        })
    }

    pub(super) fn dead_scratch(&mut self, words: u16, max: u16) -> Option<u16> {
        for original in 0..self.code.registers_size {
            let start = self.shift(original);
            if start > max {
                break;
            }
            let Some(end) = original.checked_add(words) else {
                continue;
            };
            if end > self.code.registers_size {
                break;
            }
            if (original..end).enumerate().all(|(word, reg)| {
                let shifted = self.shift(reg);
                shifted == start + word as u16
                    && !self.used[shifted as usize]
                    && !self.liveness.is_live(self.index, reg)
            }) {
                self.used[start as usize..start as usize + words as usize].fill(true);
                return Some(start);
            }
        }
        None
    }
}
