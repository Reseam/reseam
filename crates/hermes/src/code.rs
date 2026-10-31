// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::ModuleId;

/// Where a hook's code runs in an app function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookKind {
    /// Runs on entry; falling off the end calls the function with its arguments, as assigned.
    Before,
    /// Runs after the function returns; [`Operand::Result`] is its result, which falling off
    /// the end returns.
    After,
    /// Runs instead of the function; falling off the end returns `undefined`.
    Replace,
}

/// The code of a hook: blocks of statements, the first being the hook's body. Blocks run only
/// where a [`Statement::Branch`] names them. Export names and texts are `S`: written as text, and
/// resolved to the file's strings when the hook is compiled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Code<S = String> {
    pub blocks: Vec<Vec<Statement<S>>>,
}

/// A block of [`Code`], by index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockId(pub u32);

/// A value a [`Statement::Call`] keeps for later statements, by index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement<S = String> {
    /// Calls `export` of a linked `module` with `this` undefined, and keeps the result in `into`.
    Call {
        into: SlotId,
        module: ModuleId,
        export: S,
        arguments: Vec<Operand<S>>,
    },
    Assign {
        place: Place,
        value: Operand<S>,
    },
    Return(Operand<S>),
    Branch {
        condition: Condition<S>,
        then: BlockId,
        otherwise: BlockId,
    },
}

/// What [`Statement::Assign`] overwrites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Param(u32),
    Result,
    Slot(SlotId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operand<S = String> {
    Receiver,
    /// The argument at an index, as assigned; `undefined` past the last.
    Param(u32),
    /// The function's result, in [`HookKind::After`] code.
    Result,
    Slot(SlotId),
    Undefined,
    Null,
    Bool(bool),
    Int(i32),
    Text(S),
}

/// A test on operands, by JavaScript's rules: truthiness, `== null`, and strict equality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition<S = String> {
    True(Operand<S>),
    False(Operand<S>),
    Null(Operand<S>),
    NotNull(Operand<S>),
    Equal(Operand<S>, Operand<S>),
    NotEqual(Operand<S>, Operand<S>),
}
