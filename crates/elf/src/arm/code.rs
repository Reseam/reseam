// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::ops::Range;

use super::decode;
use super::instruction::Flow;
use super::map::SectionCode;
use crate::error::Result;
use crate::file::ElfImage;
use crate::references::References;
use crate::{Branch, Function};

/// What the walk found: where each function's instructions lie, and the data addresses they
/// compute.
pub(crate) struct CodeMap {
    sections: Vec<SectionCode>,
    pub(crate) references: References,
}

impl CodeMap {
    /// Owns each address in `found`, which the instruction at the paired address computes, by the
    /// function whose code holds that instruction.
    pub(super) fn new(
        image: &ElfImage,
        sections: Vec<SectionCode>,
        mut found: Vec<(u64, u64)>,
    ) -> Result<Self> {
        found.sort_unstable();
        let references = References::new(found.into_iter().filter_map(|(pc, target)| {
            function(&sections, image, pc)
                .transpose()
                .map(|function| Ok((target, function?)))
        }))?;
        Ok(Self {
            sections,
            references,
        })
    }

    /// The function whose code holds `address`.
    pub(crate) fn function(&self, image: &ElfImage, address: u64) -> Result<Option<Function>> {
        function(&self.sections, image, address)
    }

    /// The conditional branches lying wholly in `range`, which one function's code holds.
    pub(crate) fn conditional_branches(
        &self,
        image: &ElfImage,
        range: Range<u64>,
    ) -> Result<Vec<Branch>> {
        let Some(section) = section(&self.sections, range.start) else {
            return Ok(Vec::new());
        };
        let code = section.bytes(image)?;
        Ok(section
            .instructions_in(range.clone())
            .filter_map(|(address, mode)| {
                let (instruction, size) = decode::read(code, section.address, mode, address)?;
                match instruction.flow {
                    Flow::Branch(target)
                        if instruction.conditional && address + size <= range.end =>
                    {
                        Some(Branch {
                            address,
                            size,
                            target,
                            instruction_set: mode.into(),
                        })
                    }
                    _ => None,
                }
            })
            .collect())
    }
}

fn section(sections: &[SectionCode], address: u64) -> Option<&SectionCode> {
    sections.iter().find(|section| section.contains(address))
}

fn function(sections: &[SectionCode], image: &ElfImage, address: u64) -> Result<Option<Function>> {
    let Some(section) = section(sections, address) else {
        return Ok(None);
    };
    Ok(section.function(section.bytes(image)?, address))
}
