// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::FunctionId;
use crate::assemble::encode;
use crate::closure::environment_depth;
use crate::code::{Code, HookKind};
use crate::edit::{EditedFunction, generated_function};
use crate::error::{Result, Unsupported, unsupported};
use crate::function::Function;
use crate::opcode::{BUILTIN_APPLY, Instruction, Op};
use crate::string::StringId;

mod statement;

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

// Registers are operand values, each checked to fit the byte a register operand holds.
const ENVIRONMENT: u64 = 0;
/// Holds `undefined` for the lazily created `arguments` object.
const LAZY_ARGUMENTS: u64 = 1;
const RECEIVER: u64 = 2;
const ARGUMENTS: u64 = 3;
const RESULT: u64 = 4;
const MODULES: u64 = 5;
const ORIGINAL: u64 = 6;
const UNDEFINED: u64 = 7;
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
            emitter.emit(Op::Ret, &[RESULT]);
        }
        HookKind::After => emitter.emit(Op::Ret, &[RESULT]),
        HookKind::Replace => emitter.emit(Op::Ret, &[UNDEFINED]),
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
    fn jump(&mut self, op: Op, label: Label, registers: &[u64]) {
        let values: Vec<u64> = std::iter::once(0)
            .chain(registers.iter().copied())
            .collect();
        self.jumps.push((self.code.len(), label));
        self.emit(op, &values);
    }

    fn register(&self, index: u32) -> Result<u64> {
        u8::try_from(index)
            .map(u64::from)
            .map_err(|_| unsupported(self.function, Unsupported::Registers))
    }

    fn temp(&mut self) -> Result<u64> {
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
        self.emit(Op::GetParentEnvironment, &[ENVIRONMENT, 0]);
        self.emit(Op::LoadConstUndefined, &[LAZY_ARGUMENTS]);
        self.emit(Op::LoadConstUndefined, &[UNDEFINED]);
        self.emit(Op::LoadConstUndefined, &[RESULT]);
        if strict {
            self.emit(Op::LoadParam, &[RECEIVER, 0]);
        } else {
            self.emit(Op::LoadThisNS, &[RECEIVER]);
        }
        for slot in 0..self.layer.slots {
            self.emit(Op::LoadConstUndefined, &[u64::from(FIRST_SLOT + slot)]);
        }
        if let Some(original) = self.links.original {
            let original = u64::from(original.0);
            self.emit(
                Op::CreateClosureLongIndex,
                &[ORIGINAL, ENVIRONMENT, original],
            );
        }
        if let Some(depth) = self.links.modules {
            let depth = u64::from(environment_depth(self.function, depth)?);
            self.emit(Op::GetParentEnvironment, &[MODULES, depth]);
        }
        // The arguments as an array, so assignments change what the original receives.
        self.emit(Op::NewArray, &[ARGUMENTS, 0]);
        let (count, index, value, one) = (self.temp()?, self.temp()?, self.temp()?, self.temp()?);
        self.emit(Op::GetArgumentsLength, &[count, LAZY_ARGUMENTS]);
        self.emit(Op::LoadConstZero, &[index]);
        self.emit(Op::LoadConstUInt8, &[one, 1]);
        let (test, done) = (self.label(), self.label());
        self.place(test);
        self.jump(Op::JGreaterEqualLong, done, &[index, count]);
        self.emit(
            Op::GetArgumentsPropByValStrict,
            &[value, index, LAZY_ARGUMENTS],
        );
        self.emit(Op::DefineOwnByVal, &[ARGUMENTS, value, index, 1]);
        self.emit(Op::Add, &[index, index, one]);
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
    fn apply(&mut self, into: u64, function: u64, arguments: u64, this: u64) {
        self.calls.push(self.code.len());
        self.emit(Op::Mov, &[0, function]);
        self.emit(Op::Mov, &[0, arguments]);
        self.emit(Op::Mov, &[0, this]);
        self.emit(Op::CallBuiltin, &[into, u64::from(BUILTIN_APPLY), 4]);
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
