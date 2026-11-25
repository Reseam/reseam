// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::ops::Range;
use std::sync::OnceLock;

use crate::error::Result;
use crate::file::{ElfImage, Machine};
use crate::references::References;
use crate::{Branch, Function, a64, arm};

/// Where the library's functions lie, and the data addresses their code computes.
pub(crate) enum Code {
    /// The unwind table bounds AArch64 functions. Their references are indexed on first use.
    A64 {
        functions: a64::FunctionTable,
        references: OnceLock<References>,
    },
    /// 32-bit ARM functions are found by walking the code, which indexes their references.
    Arm(arm::CodeMap),
}

impl Code {
    pub(crate) fn read(image: &ElfImage) -> Result<Self> {
        Ok(match image.machine {
            Machine::Aarch64 => Self::A64 {
                functions: a64::FunctionTable::read(image)?,
                references: OnceLock::new(),
            },
            Machine::Arm => Self::Arm(arm::walk(image)?),
        })
    }

    pub(crate) fn references(&self, image: &ElfImage) -> Result<&References> {
        match self {
            Self::A64 {
                functions,
                references,
            } => {
                if let Some(references) = references.get() {
                    return Ok(references);
                }
                let built = a64::references(image, functions)?;
                Ok(references.get_or_init(|| built))
            }
            Self::Arm(map) => Ok(&map.references),
        }
    }

    /// The function whose code holds `address`.
    pub(crate) fn function(&self, image: &ElfImage, address: u64) -> Result<Option<Function>> {
        match self {
            Self::A64 { functions, .. } => functions.containing(image, address),
            Self::Arm(map) => map.function(image, address),
        }
    }

    /// The conditional branches lying wholly in `range`, which one function's code holds.
    pub(crate) fn conditional_branches(
        &self,
        image: &ElfImage,
        range: Range<u64>,
    ) -> Result<Vec<Branch>> {
        match self {
            Self::A64 { .. } => a64::conditional_branches(image, range),
            Self::Arm(map) => map.conditional_branches(image, range),
        }
    }
}
