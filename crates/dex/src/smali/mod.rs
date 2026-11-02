// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! DEX code written as smali, in baksmali's syntax: mnemonics, `vN`/`pN`
//! registers, `:cond_0`-style labels and `.catch` directives.
//! [`class_declarations`] writes a whole class in the same notation.

mod body;
mod declarations;
mod flags;
mod labels;
mod mnemonic;
mod names;
mod operands;

pub use declarations::{
    ClassDeclarations, Declaration, MemberDeclaration, References, class_declarations,
};
pub use flags::{DeclarationKind, access_keywords};
pub use names::SmaliNames;

use crate::error::Result;
use crate::file::DexFile;
use crate::model::code::CodeItem;
use body::Body;

/// One line of a method body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmaliLine {
    /// A branch, switch, payload, try or handler target, such as `:cond_0`.
    Label(String),
    /// Instruction `index` of the body. Payloads span several lines of text.
    Instruction { index: usize, text: String },
    /// A `.catch` or `.catchall` directive, after the try range it closes.
    Directive(String),
}

/// The body of a method of `dex` as smali lines, labels and directives where
/// baksmali puts them.
pub fn disassemble(dex: &DexFile, code: &CodeItem) -> Result<Vec<SmaliLine>> {
    Body::new(dex, code)?.lines()
}
