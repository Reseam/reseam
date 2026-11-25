// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{exidx, relocations};
use crate::error::Result;
use crate::file::ElfImage;

/// What the library records about its functions, where the walk starts.
pub(super) struct Seeds {
    /// Function starts without an instruction set, from the unwind table.
    pub(super) starts: Vec<u64>,
    /// Pointers to functions, with the Thumb bit: symbol values and relocated pointers.
    pub(super) pointers: Vec<u64>,
}

impl Seeds {
    pub(super) fn read(image: &ElfImage) -> Result<Self> {
        let mut pointers = image
            .function_symbols()?
            .map(|symbol| Ok(symbol?.value))
            .collect::<Result<Vec<_>>>()?;
        pointers.extend(relocations::relative_values(image)?);
        Ok(Self {
            starts: exidx::function_starts(image)?,
            pointers,
        })
    }
}
