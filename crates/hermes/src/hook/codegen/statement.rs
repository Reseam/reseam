// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{ARGUMENTS, Emitter, FIRST_SLOT, Label, MODULES, RECEIVER, RESULT, UNDEFINED};
use crate::code::{Condition, Operand, Place, Statement};
use crate::error::Result;
use crate::opcode::Op;
use crate::string::StringId;

impl Emitter<'_> {
    pub(super) fn block(&mut self, id: u32) -> Result<()> {
        let layer = self.layer;
        for statement in &layer.code.blocks[id as usize] {
            self.temps = FIRST_SLOT + layer.slots;
            match statement {
                Statement::Call {
                    into,
                    module,
                    export,
                    arguments,
                } => {
                    let into = self.register(FIRST_SLOT + into.0)?;
                    self.call(into, module.0, *export, arguments)?;
                }
                Statement::Assign { place, value } => {
                    let value = self.load(value)?;
                    match place {
                        Place::Param(index) => self.emit(
                            Op::DefineOwnByIndexL,
                            &[ARGUMENTS, value, u64::from(*index)],
                        ),
                        Place::Result => self.emit(Op::Mov, &[RESULT, value]),
                        Place::Slot(slot) => {
                            let slot = self.register(FIRST_SLOT + slot.0)?;
                            self.emit(Op::Mov, &[slot, value]);
                        }
                    }
                }
                Statement::Return(value) => {
                    let value = self.load(value)?;
                    self.emit(Op::Ret, &[value]);
                }
                Statement::Branch {
                    condition,
                    then,
                    otherwise,
                } => {
                    let (otherwise_label, end) = (self.label(), self.label());
                    self.test(condition, otherwise_label)?;
                    self.block(then.0)?;
                    self.jump(Op::JmpLong, end, &[]);
                    self.place(otherwise_label);
                    self.block(otherwise.0)?;
                    self.place(end);
                }
            }
        }
        Ok(())
    }

    /// Jumps to `otherwise` unless `condition` holds.
    fn test(&mut self, condition: &Condition<StringId>, otherwise: Label) -> Result<()> {
        match condition {
            Condition::True(value) => {
                let value = self.load(value)?;
                self.jump(Op::JmpFalseLong, otherwise, &[value]);
            }
            Condition::False(value) => {
                let value = self.load(value)?;
                self.jump(Op::JmpTrueLong, otherwise, &[value]);
            }
            Condition::Null(value) | Condition::NotNull(value) => {
                let value = self.load(value)?;
                let null = self.temp()?;
                self.emit(Op::LoadConstNull, &[null]);
                let op = if matches!(condition, Condition::Null(_)) {
                    Op::JNotEqualLong
                } else {
                    Op::JEqualLong
                };
                self.jump(op, otherwise, &[value, null]);
            }
            Condition::Equal(left, right) | Condition::NotEqual(left, right) => {
                let (left, right) = (self.load(left)?, self.load(right)?);
                let op = if matches!(condition, Condition::Equal(..)) {
                    Op::JStrictNotEqualLong
                } else {
                    Op::JStrictEqualLong
                };
                self.jump(op, otherwise, &[left, right]);
            }
        }
        Ok(())
    }

    /// Calls the export `name` of the module in environment slot `module`, with `this` undefined.
    fn call(
        &mut self,
        into: u64,
        module: u32,
        name: StringId,
        arguments: &[Operand<StringId>],
    ) -> Result<()> {
        let function = self.temp()?;
        self.emit(
            Op::LoadFromEnvironmentL,
            &[function, MODULES, u64::from(module)],
        );
        let cache = self.cache()?;
        self.emit(
            Op::GetByIdLong,
            &[function, function, cache, u64::from(name.0)],
        );
        let array = self.temp()?;
        self.emit(Op::NewArray, &[array, 0]);
        for (position, argument) in arguments.iter().enumerate() {
            let value = self.load(argument)?;
            self.emit(Op::DefineOwnByIndexL, &[array, value, position as u64]);
        }
        self.apply(into, function, array, UNDEFINED);
        Ok(())
    }

    /// The register holding `value`, loading it into a scratch register when it has none.
    fn load(&mut self, value: &Operand<StringId>) -> Result<u64> {
        let (op, operand) = match value {
            Operand::Receiver => return Ok(RECEIVER),
            Operand::Result => return Ok(RESULT),
            Operand::Undefined => return Ok(UNDEFINED),
            Operand::Slot(slot) => return self.register(FIRST_SLOT + slot.0),
            Operand::Param(index) => {
                let value = self.temp()?;
                let key = self.temp()?;
                self.emit(Op::LoadConstInt, &[key, u64::from(*index)]);
                self.emit(Op::GetByVal, &[value, ARGUMENTS, key]);
                return Ok(value);
            }
            Operand::Null => (Op::LoadConstNull, None),
            Operand::Bool(true) => (Op::LoadConstTrue, None),
            Operand::Bool(false) => (Op::LoadConstFalse, None),
            Operand::Int(value) => (Op::LoadConstInt, Some(u64::from(value.cast_unsigned()))),
            Operand::Text(text) => (Op::LoadConstStringLongIndex, Some(u64::from(text.0))),
        };
        let register = self.temp()?;
        let values: Vec<u64> = std::iter::once(register).chain(operand).collect();
        self.emit(op, &values);
        Ok(register)
    }
}
