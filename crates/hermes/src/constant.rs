// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::FunctionId;
use crate::assemble::encode;
use crate::edit::Editor;
use crate::error::{HermesError, Result};
use crate::link::generated_function;
use crate::model::StringKind;
use crate::opcode::{Instruction, Op};

/// A value a replaced function returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Constant {
    Undefined,
    Null,
    Bool(bool),
    Int(i32),
    Text(String),
}

impl Editor<'_> {
    /// Replaces the body of `function` with `return value`; callers are untouched. Earlier wraps
    /// of the function no longer run, and later wraps receive the constant body as `original`.
    /// Generators, async functions and constructors fail, as do functions whose closures a wrap
    /// still needs.
    pub fn always_return(&mut self, function: FunctionId, value: &Constant) -> Result<()> {
        self.transaction(|editor| editor.return_constant(function, value))
    }

    fn return_constant(&mut self, function: FunctionId, value: &Constant) -> Result<()> {
        let target = self.editable(function)?;
        let (name, parameters, strict) = (
            target.name(),
            target.header.parameters,
            target.header.flags.strict,
        );
        if self.edits.roots.contains_key(&function) && !self.edits.relocated.contains_key(&function)
        {
            return Err(HermesError::Unsupported(format!(
                "function {} creates closures that a wrap needs",
                function.0
            )));
        }
        let load = match value {
            Constant::Undefined => Instruction::new(Op::LoadConstUndefined, &[0]),
            Constant::Null => Instruction::new(Op::LoadConstNull, &[0]),
            Constant::Bool(true) => Instruction::new(Op::LoadConstTrue, &[0]),
            Constant::Bool(false) => Instruction::new(Op::LoadConstFalse, &[0]),
            Constant::Int(value) => {
                Instruction::new(Op::LoadConstInt, &[0, u64::from(value.cast_unsigned())])
            }
            Constant::Text(text) => Instruction::new(
                Op::LoadConstStringLongIndex,
                &[0, u64::from(self.intern(text, StringKind::String)?.0)],
            ),
        };
        let mut body = generated_function(
            name,
            parameters,
            1,
            encode(&[load, Instruction::new(Op::Ret, &[0])]),
        );
        body.header.flags.strict = strict;
        self.edits.functions.insert(function, body);
        if !self.edits.relocated.contains_key(&function) {
            self.edits.discarded.insert(function);
        }
        Ok(())
    }
}
