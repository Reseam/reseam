// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::declared_export;
use crate::FunctionId;
use crate::HermesFile;
use crate::code::{Code, Condition, HookKind, Operand, Place, SlotId, Statement};
use crate::edit::Edits;
use crate::error::{HermesError, Result, Unsupported, unsupported};
use crate::string::{StringId, StringKind};

/// Checks a hook's code and resolves its names to the file's strings.
pub(super) struct Resolver<'e, 'a> {
    pub(super) function: FunctionId,
    pub(super) file: &'e HermesFile<'a>,
    pub(super) edits: &'e mut Edits,
    pub(super) kind: HookKind,
    pub(super) slots: u32,
    pub(super) calls_exports: bool,
}

impl Resolver<'_, '_> {
    pub(super) fn code(&mut self, code: &Code) -> Result<Code<StringId>> {
        if code.blocks.is_empty() {
            return Err(self.malformed("hook code has no body"));
        }
        let mut entered = vec![false; code.blocks.len()];
        entered[0] = true;
        for statement in code.blocks.iter().flatten() {
            if let Statement::Branch {
                then, otherwise, ..
            } = statement
            {
                for block in [then, otherwise] {
                    let entry = entered
                        .get_mut(block.0 as usize)
                        .ok_or_else(|| self.malformed("branch to a missing block"))?;
                    if std::mem::replace(entry, true) {
                        return Err(self.malformed("a block runs from more than one place"));
                    }
                }
            }
        }
        let blocks = code
            .blocks
            .iter()
            .map(|block| block.iter().map(|s| self.statement(s)).collect())
            .collect::<Result<_>>()?;
        Ok(Code { blocks })
    }

    fn statement(&mut self, statement: &Statement) -> Result<Statement<StringId>> {
        Ok(match statement {
            Statement::Call {
                into,
                module,
                export,
                arguments,
            } => {
                self.slot(*into);
                self.calls_exports = true;
                Statement::Call {
                    into: *into,
                    module: *module,
                    export: declared_export(self.file, self.edits, self.function, *module, export)?,
                    arguments: arguments
                        .iter()
                        .map(|argument| self.operand(argument))
                        .collect::<Result<_>>()?,
                }
            }
            Statement::Assign { place, value } => {
                match place {
                    Place::Result => self.require_result()?,
                    Place::Slot(slot) => self.slot(*slot),
                    Place::Param(_) => {}
                }
                Statement::Assign {
                    place: *place,
                    value: self.operand(value)?,
                }
            }
            Statement::Return(value) => Statement::Return(self.operand(value)?),
            Statement::Branch {
                condition,
                then,
                otherwise,
            } => Statement::Branch {
                condition: match condition {
                    Condition::True(value) => Condition::True(self.operand(value)?),
                    Condition::False(value) => Condition::False(self.operand(value)?),
                    Condition::Null(value) => Condition::Null(self.operand(value)?),
                    Condition::NotNull(value) => Condition::NotNull(self.operand(value)?),
                    Condition::Equal(left, right) => {
                        Condition::Equal(self.operand(left)?, self.operand(right)?)
                    }
                    Condition::NotEqual(left, right) => {
                        Condition::NotEqual(self.operand(left)?, self.operand(right)?)
                    }
                },
                then: *then,
                otherwise: *otherwise,
            },
        })
    }

    fn operand(&mut self, operand: &Operand) -> Result<Operand<StringId>> {
        Ok(match operand {
            Operand::Receiver => Operand::Receiver,
            Operand::Param(index) => Operand::Param(*index),
            Operand::Result => {
                self.require_result()?;
                Operand::Result
            }
            Operand::Slot(slot) => {
                self.slot(*slot);
                Operand::Slot(*slot)
            }
            Operand::Undefined => Operand::Undefined,
            Operand::Null => Operand::Null,
            Operand::Bool(value) => Operand::Bool(*value),
            Operand::Int(value) => Operand::Int(*value),
            Operand::Text(text) => Operand::Text(self.edits.strings_mut().intern(
                self.file,
                text,
                StringKind::String,
            )?),
        })
    }

    fn malformed(&self, reason: &'static str) -> HermesError {
        unsupported(self.function, Unsupported::Malformed(reason))
    }

    fn slot(&mut self, slot: SlotId) {
        self.slots = self.slots.max(slot.0 + 1);
    }

    fn require_result(&self) -> Result<()> {
        if self.kind == HookKind::After {
            Ok(())
        } else {
            Err(self.malformed("only after hooks have a result"))
        }
    }
}
