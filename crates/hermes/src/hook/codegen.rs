// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::FunctionId;
use crate::assemble::encode;
use crate::closure::environment_depth;
use crate::code::{Code, Condition, HookKind, Operand, Place, Statement};
use crate::edit::EditedFunction;
use crate::edit::generated_function;
use crate::error::{Result, Unsupported, unsupported};
use crate::function::Function;
use crate::opcode::{BUILTIN_APPLY, Instruction, Op};
use crate::string::StringId;

/// A hook ready to compile: its code with names resolved to the file's strings, and how many
/// slots it keeps.
pub(crate) struct Layer {
    pub kind: HookKind,
    pub code: Code<StringId>,
    pub slots: u32,
}

/// The functions a compiled layer reaches beyond its own body.
pub(crate) struct Links {
    /// The body before this layer, unless the layer replaces it.
    pub original: Option<FunctionId>,
    /// The depth of the linked modules' private environment, when the layer calls an export.
    pub modules: Option<u16>,
}

const ENVIRONMENT: u8 = 0;
/// Holds `undefined` for the lazily created `arguments` object.
const LAZY_ARGUMENTS: u8 = 1;
const RECEIVER: u8 = 2;
const ARGUMENTS: u8 = 3;
const RESULT: u8 = 4;
const MODULES: u8 = 5;
const ORIGINAL: u8 = 6;
const UNDEFINED: u8 = 7;
const FIRST_SLOT: u32 = 8;
/// A call's outgoing registers sit below the frame's last eight: `this` at `frame - 8`, then
/// each argument below it.
const CALL_FRAME: u32 = 8;

/// Compiles `layer` into the body of `function`.
pub(crate) fn compile(
    function: &Function<'_>,
    layer: &Layer,
    links: &Links,
) -> Result<EditedFunction> {
    let mut emitter = Emitter {
        function: function.id,
        code: Vec::new(),
        labels: Vec::new(),
        jumps: Vec::new(),
        calls: Vec::new(),
        temps: FIRST_SLOT + layer.slots,
        top: FIRST_SLOT + layer.slots,
        read_cache: 0,
        layer,
        links,
    };
    emitter.prologue(function.header.flags.strict)?;
    if layer.kind == HookKind::After {
        emitter.call_original();
    }
    emitter.block(0)?;
    match layer.kind {
        HookKind::Before => {
            emitter.call_original();
            emitter.emit(Op::Ret, &[u64::from(RESULT)]);
        }
        HookKind::After => emitter.emit(Op::Ret, &[u64::from(RESULT)]),
        HookKind::Replace => emitter.emit(Op::Ret, &[u64::from(UNDEFINED)]),
    }
    let read_cache = emitter.read_cache;
    let (body, frame) = emitter.finish()?;
    let mut result = generated_function(function.name(), function.header.parameters, frame, body);
    result.header.read_cache = read_cache;
    result.header.flags.strict = function.header.flags.strict;
    Ok(result)
}

#[derive(Clone, Copy)]
struct Label(usize);

struct Emitter<'a> {
    function: FunctionId,
    code: Vec<Instruction>,
    /// The instruction index each label marks.
    labels: Vec<Option<usize>>,
    /// Jump instructions and the label each one targets.
    jumps: Vec<(usize, Label)>,
    /// The first of the three moves that stage each call's outgoing registers.
    calls: Vec<usize>,
    /// The next free scratch register.
    temps: u32,
    /// One past the highest register used.
    top: u32,
    read_cache: u8,
    layer: &'a Layer,
    links: &'a Links,
}

impl Emitter<'_> {
    fn emit(&mut self, op: Op, values: &[u64]) {
        self.code.push(Instruction::new(op, values));
    }

    fn label(&mut self) -> Label {
        self.labels.push(None);
        Label(self.labels.len() - 1)
    }

    fn place(&mut self, label: Label) {
        self.labels[label.0] = Some(self.code.len());
    }

    /// Emits a jump whose first operand, its offset, is resolved when the code is finished.
    fn jump(&mut self, op: Op, label: Label, registers: &[u8]) {
        let values: Vec<u64> = std::iter::once(0)
            .chain(registers.iter().map(|&r| u64::from(r)))
            .collect();
        self.jumps.push((self.code.len(), label));
        self.emit(op, &values);
    }

    fn register(&self, index: u32) -> Result<u8> {
        u8::try_from(index).map_err(|_| unsupported(self.function, Unsupported::Registers))
    }

    fn temp(&mut self) -> Result<u8> {
        let temp = self.register(self.temps)?;
        self.temps += 1;
        self.top = self.top.max(self.temps);
        Ok(temp)
    }

    fn cache(&mut self) -> Result<u64> {
        let index = self.read_cache;
        self.read_cache = index
            .checked_add(1)
            .ok_or_else(|| unsupported(self.function, Unsupported::Properties))?;
        Ok(u64::from(index))
    }

    fn prologue(&mut self, strict: bool) -> Result<()> {
        self.emit(Op::GetParentEnvironment, &[u64::from(ENVIRONMENT), 0]);
        self.emit(Op::LoadConstUndefined, &[u64::from(LAZY_ARGUMENTS)]);
        self.emit(Op::LoadConstUndefined, &[u64::from(UNDEFINED)]);
        self.emit(Op::LoadConstUndefined, &[u64::from(RESULT)]);
        if strict {
            self.emit(Op::LoadParam, &[u64::from(RECEIVER), 0]);
        } else {
            self.emit(Op::LoadThisNS, &[u64::from(RECEIVER)]);
        }
        for slot in 0..self.layer.slots {
            self.emit(Op::LoadConstUndefined, &[u64::from(FIRST_SLOT + slot)]);
        }
        if let Some(original) = self.links.original {
            self.emit(
                Op::CreateClosureLongIndex,
                &[
                    u64::from(ORIGINAL),
                    u64::from(ENVIRONMENT),
                    u64::from(original.0),
                ],
            );
        }
        if let Some(depth) = self.links.modules {
            self.emit(
                Op::GetParentEnvironment,
                &[
                    u64::from(MODULES),
                    u64::from(environment_depth(self.function, depth)?),
                ],
            );
        }
        // The arguments as an array, so assignments change what the original receives.
        self.emit(Op::NewArray, &[u64::from(ARGUMENTS), 0]);
        let (count, index, value, one) = (self.temp()?, self.temp()?, self.temp()?, self.temp()?);
        self.emit(
            Op::GetArgumentsLength,
            &[u64::from(count), u64::from(LAZY_ARGUMENTS)],
        );
        self.emit(Op::LoadConstZero, &[u64::from(index)]);
        self.emit(Op::LoadConstUInt8, &[u64::from(one), 1]);
        let (test, done) = (self.label(), self.label());
        self.place(test);
        self.jump(Op::JGreaterEqualLong, done, &[index, count]);
        self.emit(
            Op::GetArgumentsPropByValStrict,
            &[
                u64::from(value),
                u64::from(index),
                u64::from(LAZY_ARGUMENTS),
            ],
        );
        self.emit(
            Op::DefineOwnByVal,
            &[u64::from(ARGUMENTS), u64::from(value), u64::from(index), 1],
        );
        self.emit(
            Op::Add,
            &[u64::from(index), u64::from(index), u64::from(one)],
        );
        self.jump(Op::JmpLong, test, &[]);
        self.place(done);
        self.temps = FIRST_SLOT + self.layer.slots;
        Ok(())
    }

    fn call_original(&mut self) {
        self.apply(RESULT, ORIGINAL, ARGUMENTS, RECEIVER);
    }

    /// Calls `function` with the array `arguments` and receiver `this`. The outgoing registers
    /// depend on the frame size, so [`Self::finish`] fills them in.
    fn apply(&mut self, into: u8, function: u8, arguments: u8, this: u8) {
        self.calls.push(self.code.len());
        self.emit(Op::Mov, &[0, u64::from(function)]);
        self.emit(Op::Mov, &[0, u64::from(arguments)]);
        self.emit(Op::Mov, &[0, u64::from(this)]);
        self.emit(
            Op::CallBuiltin,
            &[u64::from(into), u64::from(BUILTIN_APPLY), 4],
        );
    }

    fn block(&mut self, id: u32) -> Result<()> {
        let layer = self.layer;
        for statement in &layer.code.blocks[id as usize] {
            self.temps = FIRST_SLOT + layer.slots;
            match statement {
                Statement::Call {
                    into,
                    module,
                    export,
                    arguments,
                } => self.call(
                    self.register(FIRST_SLOT + into.0)?,
                    module.0,
                    *export,
                    arguments,
                )?,
                Statement::Assign { place, value } => {
                    let value = self.load(value)?;
                    match place {
                        Place::Param(index) => self.emit(
                            Op::DefineOwnByIndexL,
                            &[u64::from(ARGUMENTS), u64::from(value), u64::from(*index)],
                        ),
                        Place::Result => self.emit(Op::Mov, &[u64::from(RESULT), u64::from(value)]),
                        Place::Slot(slot) => {
                            let slot = self.register(FIRST_SLOT + slot.0)?;
                            self.emit(Op::Mov, &[u64::from(slot), u64::from(value)]);
                        }
                    }
                }
                Statement::Return(value) => {
                    let value = self.load(value)?;
                    self.emit(Op::Ret, &[u64::from(value)]);
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
                self.emit(Op::LoadConstNull, &[u64::from(null)]);
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
        into: u8,
        module: u32,
        name: StringId,
        arguments: &[Operand<StringId>],
    ) -> Result<()> {
        let function = self.temp()?;
        self.emit(
            Op::LoadFromEnvironmentL,
            &[u64::from(function), u64::from(MODULES), u64::from(module)],
        );
        let cache = self.cache()?;
        self.emit(
            Op::GetByIdLong,
            &[
                u64::from(function),
                u64::from(function),
                cache,
                u64::from(name.0),
            ],
        );
        let array = self.temp()?;
        self.emit(Op::NewArray, &[u64::from(array), 0]);
        for (position, argument) in arguments.iter().enumerate() {
            let value = self.load(argument)?;
            self.emit(
                Op::DefineOwnByIndexL,
                &[u64::from(array), u64::from(value), position as u64],
            );
        }
        self.apply(into, function, array, UNDEFINED);
        Ok(())
    }

    /// The register holding `value`, loading it into a scratch register when it has none.
    fn load(&mut self, value: &Operand<StringId>) -> Result<u8> {
        let constant = |emitter: &mut Self, op: Op, operand: Option<u64>| -> Result<u8> {
            let register = emitter.temp()?;
            let mut values = vec![u64::from(register)];
            values.extend(operand);
            emitter.emit(op, &values);
            Ok(register)
        };
        match value {
            Operand::Receiver => Ok(RECEIVER),
            Operand::Result => Ok(RESULT),
            Operand::Undefined => Ok(UNDEFINED),
            Operand::Slot(slot) => self.register(FIRST_SLOT + slot.0),
            Operand::Param(index) => {
                let value = self.temp()?;
                let key = self.temp()?;
                self.emit(Op::LoadConstInt, &[u64::from(key), u64::from(*index)]);
                self.emit(
                    Op::GetByVal,
                    &[u64::from(value), u64::from(ARGUMENTS), u64::from(key)],
                );
                Ok(value)
            }
            Operand::Null => constant(self, Op::LoadConstNull, None),
            Operand::Bool(true) => constant(self, Op::LoadConstTrue, None),
            Operand::Bool(false) => constant(self, Op::LoadConstFalse, None),
            Operand::Int(value) => constant(
                self,
                Op::LoadConstInt,
                Some(u64::from(value.cast_unsigned())),
            ),
            Operand::Text(text) => {
                constant(self, Op::LoadConstStringLongIndex, Some(u64::from(text.0)))
            }
        }
    }

    /// Resolves jumps and outgoing registers; returns the body and its frame size.
    fn finish(mut self) -> Result<(Vec<u8>, u32)> {
        let frame = self.top + CALL_FRAME + 3;
        self.register(frame)?;
        for &first in &self.calls {
            for offset in 0..3 {
                self.code[first + offset].values[0] =
                    u64::from(frame - CALL_FRAME - 1 - offset as u32);
            }
        }
        let positions: Vec<u32> = self
            .code
            .iter()
            .scan(0, |position, inst| {
                let start = *position;
                *position += inst.definition().size() as u32;
                Some(start)
            })
            .collect();
        for &(at, label) in &self.jumps {
            let target = self.labels[label.0].expect("every label is placed");
            let end = positions.len();
            let target_position = if target == end {
                positions[end - 1] + self.code[end - 1].definition().size() as u32
            } else {
                positions[target]
            };
            self.code[at].values[0] = u64::from(target_position.wrapping_sub(positions[at]));
        }
        Ok((encode(&self.code), frame))
    }
}
