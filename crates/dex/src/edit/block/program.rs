// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::lifetimes::{Temporary, temporaries};
use super::planned::{Planned, plan};
use super::{BlockRegister, BlockStep};
use crate::DexFile;
use crate::error::Result;

/// A block's instructions with what each does to its operands, and the span each temporary
/// holds a value over.
pub(super) struct Program {
    pub(super) instructions: Vec<Planned>,
    pub(super) temporaries: Vec<Option<Temporary>>,
}

impl Program {
    pub(super) fn new(steps: &[BlockStep], dex: &DexFile) -> Result<Self> {
        let instructions = plan(steps, dex)?;
        let temporaries = temporaries(&instructions)?;
        Ok(Self {
            instructions,
            temporaries,
        })
    }

    /// Every word of every method register the block names.
    pub(super) fn fixed_registers(&self) -> impl Iterator<Item = u16> + '_ {
        self.instructions
            .iter()
            .flat_map(|planned| &planned.operands)
            .filter_map(|operand| match operand.register {
                BlockRegister::Fixed(register) => Some((register, operand.kind)),
                BlockRegister::Temporary(_) => None,
            })
            .flat_map(|(register, kind)| (0..kind.words()).map(move |word| register + word))
    }
}
