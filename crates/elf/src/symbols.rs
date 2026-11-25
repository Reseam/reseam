// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_bytes::{u16_at, u32_at};

use crate::arm::code_pointer;
use crate::error::{Result, invalid};
use crate::file::{ElfImage, Machine, name};
use crate::{Function, InstructionSet};

const SECTION_SYMBOLS: u32 = 2;
const SECTION_DYNAMIC_SYMBOLS: u32 = 11;
const TYPE_FUNCTION: u8 = 2;
const UNDEFINED: u16 = 0;

/// A function the dynamic or static symbol table defines.
pub(crate) struct FunctionSymbol<'a> {
    pub(crate) name: &'a [u8],
    /// The function's address, with the Thumb bit on 32-bit ARM.
    pub(crate) value: u64,
    pub(crate) size: u64,
}

impl ElfImage {
    /// The functions the library defines under `symbol`, in its dynamic and static symbol tables.
    pub(crate) fn symbol_functions(&self, symbol: &str) -> Result<Vec<Function>> {
        let mut functions = Vec::new();
        for entry in self.function_symbols()? {
            let entry = entry?;
            let function = self.function(&entry);
            if entry.name == symbol.as_bytes() && !functions.contains(&function) {
                functions.push(function);
            }
        }
        Ok(functions)
    }

    /// Every function the dynamic and static symbol tables define.
    pub(crate) fn function_symbols(
        &self,
    ) -> Result<impl Iterator<Item = Result<FunctionSymbol<'_>>>> {
        let class = self.class;
        let fields = &class.layout().symbol;
        let tables = self
            .sections()
            .iter()
            .filter(|section| matches!(section.kind, SECTION_SYMBOLS | SECTION_DYNAMIC_SYMBOLS))
            .map(|table| {
                let names = self
                    .sections()
                    .get(table.link as usize)
                    .ok_or_else(|| invalid(0, "symbol table links past the section headers"))?;
                Ok((self.contents(names)?, self.contents(table)?))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(tables.into_iter().flat_map(move |(names, entries)| {
            entries
                .chunks_exact(fields.size)
                .filter(|entry| {
                    entry[fields.info] & 0xf == TYPE_FUNCTION
                        && u16_at(entry, fields.section) != UNDEFINED
                })
                .map(move |entry| {
                    Ok(FunctionSymbol {
                        name: name(names, u32_at(entry, 0) as usize)?,
                        value: class.word_at(entry, fields.value),
                        size: class.word_at(entry, fields.length),
                    })
                })
        }))
    }

    /// The function a symbol describes.
    fn function(&self, symbol: &FunctionSymbol<'_>) -> Function {
        let (address, instruction_set) = match self.machine {
            Machine::Aarch64 => (symbol.value, InstructionSet::A64),
            Machine::Arm => {
                let (address, mode) = code_pointer(symbol.value);
                (address, mode.into())
            }
        };
        Function {
            address,
            size: symbol.size,
            instruction_set,
        }
    }
}
