// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! AArch64 libraries. The unwind tables bound every function, and code computes a data address
//! with `adr` or `adrp`, immediate adds and loads.

mod branch;
mod decode;
mod emit;
mod scan;
mod unwind;

pub(crate) use branch::{NOP, conditional_branches, jump};
pub(crate) use emit::return_code;
use scan::data_references;
pub(crate) use unwind::FunctionTable;

use crate::Function;
use crate::error::Result;
use crate::file::ElfImage;
use crate::references::References;

/// The data addresses the library's code computes, each with the function that the unwind
/// table says holds the instruction.
pub(crate) fn references(image: &ElfImage, functions: &FunctionTable) -> Result<References> {
    let mut current: Option<Function> = None;
    References::new(
        data_references(image, functions, &image.loaded_data())?
            .into_iter()
            .map(|(pc, target)| {
                if !current.is_some_and(|function| function.contains(pc)) {
                    current = functions.containing(image, pc)?;
                }
                Ok(current.map(|function| (target, function)))
            })
            .filter_map(Result::transpose),
    )
}
