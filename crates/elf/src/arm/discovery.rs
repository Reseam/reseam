// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Finding every function and its instructions by following the code. The walk starts from what
//! the library records: symbols and relocated pointers with their instruction set, and the
//! function starts of the unwind table. It follows branches and table branches within a function,
//! and takes call targets and pointers the code computes as further functions. Literal pools and
//! branch tables are never decoded, since only code that execution reaches is. Registers are
//! followed along the branches, so a block starts with what they held where control came from
//! first.

use std::ops::Range;

use super::blocks::Blocks;
use super::code::CodeMap;
use super::decode;
use super::instruction::{Effect, Flow, Instruction};
use super::map::SectionCode;
use super::mode::{Mode, code_pointer};
use super::seeds::Seeds;
use super::tracking::{Computed, Tracker};
use crate::data::LoadedData;
use crate::error::Result;
use crate::file::ElfImage;

/// Every function the library's code reaches, and the data addresses it computes.
pub(crate) fn walk(image: &ElfImage) -> Result<CodeMap> {
    let mut walk = Walk::new(image)?;
    walk.functions()?;
    CodeMap::new(image, walk.sections, walk.found)
}

/// The function being walked: the section its code is in, the range its code may lie in, its
/// instruction set, and its blocks waiting to be walked.
struct FunctionWalk {
    section: usize,
    range: Range<u64>,
    mode: Mode,
    blocks: Blocks,
}

struct Walk<'a> {
    image: &'a ElfImage,
    sections: Vec<SectionCode>,
    data: LoadedData,
    /// Functions found and not yet walked.
    entries: Vec<(u64, Mode)>,
    /// Each instruction that computes an address in `data`, with that address.
    found: Vec<(u64, u64)>,
}

impl<'a> Walk<'a> {
    /// A walk seeded with what the library records.
    fn new(image: &'a ElfImage) -> Result<Self> {
        let mut walk = Self {
            image,
            sections: image
                .sections()
                .iter()
                .enumerate()
                .filter(|(_, section)| section.is_code())
                .map(|(index, section)| SectionCode::new(index, section))
                .collect(),
            data: image.loaded_data(),
            entries: Vec::new(),
            found: Vec::new(),
        };
        let seeds = Seeds::read(image)?;
        for start in seeds.starts {
            if let Some(section) = walk.section(start) {
                walk.sections[section].insert_start(start);
            }
        }
        for pointer in seeds.pointers {
            walk.pointer(pointer);
        }
        Ok(walk)
    }

    /// Walks every function found, and those their code leads to.
    fn functions(&mut self) -> Result<()> {
        while let Some((address, mode)) = self.entries.pop() {
            self.function(address, mode)?;
        }
        Ok(())
    }

    fn section(&self, address: u64) -> Option<usize> {
        self.sections
            .iter()
            .position(|section| section.contains(address))
    }

    /// Takes a pointer into the code as a function.
    fn pointer(&mut self, value: u64) {
        let (address, mode) = code_pointer(value);
        self.entry(address, mode);
    }

    /// Records a function at `address`, to walk unless its code was walked already.
    fn entry(&mut self, address: u64, mode: Mode) {
        if address.is_multiple_of(mode.alignment())
            && let Some(section) = self.section(address)
        {
            self.sections[section].insert_start(address);
            if !self.sections[section].is_instruction(address) {
                self.entries.push((address, mode));
            }
        }
    }

    /// Decodes the function at `entry` and every block its branches reach. A function's code is
    /// contiguous, so its blocks lie between its entry and the next known start.
    fn function(&mut self, entry: u64, mode: Mode) -> Result<()> {
        let Some(section) = self
            .section(entry)
            .filter(|&section| !self.sections[section].is_instruction(entry))
        else {
            return Ok(());
        };
        let mut function = FunctionWalk {
            section,
            range: entry..self.sections[section].next_start(entry),
            mode,
            blocks: Blocks::new(entry),
        };
        while let Some((start, tracker)) = function.blocks.pop_lowest() {
            if start.is_multiple_of(mode.alignment()) && function.range.contains(&start) {
                self.block(&mut function, start, tracker)?;
            }
        }
        Ok(())
    }

    /// Decodes from `start` until control leaves, stopping at code decoded before, at another
    /// function's start, or at an instruction that would leave the function.
    fn block(
        &mut self,
        function: &mut FunctionWalk,
        start: u64,
        mut tracker: Tracker,
    ) -> Result<()> {
        let section = function.section;
        let code = self.sections[section].bytes(self.image)?;
        let mut guarded = 0u8;
        let mut restored = false;
        let mut pc = start;
        loop {
            if pc != start && self.sections[section].is_start(pc) {
                return Ok(());
            }
            let Some((instruction, size)) =
                decode::read(code, self.sections[section].address, function.mode, pc)
            else {
                return Ok(());
            };
            if pc + size > function.range.end || !self.sections[section].is_free(pc, size) {
                return Ok(());
            }
            self.sections[section].take_instruction(pc, size, function.mode);
            let conditional = instruction.conditional || guarded > 0;
            guarded = guarded.saturating_sub(1);
            self.track(&mut tracker, pc, &instruction);
            let falls_through = match instruction.flow {
                Flow::Next => true,
                Flow::IfThen { count } => {
                    guarded = count;
                    true
                }
                Flow::Branch(target) => {
                    let tail_call = !conditional
                        && restored
                        && target > pc
                        && !function.blocks.contains(target)
                        && !self.sections[section].is_instruction(target);
                    self.branch(function, target, conditional, tail_call, &tracker)
                }
                Flow::Call(target, mode) => {
                    self.entry(target, mode);
                    tracker.call();
                    true
                }
                Flow::IndirectCall => {
                    tracker.call();
                    true
                }
                Flow::Table { base, halfwords } => {
                    for target in self.sections[section].take_table(code, base, halfwords) {
                        function.blocks.insert(target, &tracker);
                    }
                    false
                }
                Flow::Stop => conditional,
            };
            if !falls_through {
                return Ok(());
            }
            restored = instruction.restores_link;
            pc += size;
        }
    }

    /// Follows a branch to `target`, and returns whether control can also fall through.
    ///
    /// An unconditional branch out of the function is a tail call into another one, as is a
    /// `tail_call`: a forward one right after an epilogue restores the link register, to code the
    /// function has not reached otherwise, which is a function the linker placed next. A
    /// shrink-wrapped function also branches after its epilogue, but to a block its frameless
    /// path reaches too, which blocks in address order have walked or queued by then. Compilers
    /// emit no conditional tail calls for ARM, so a conditional branch out of the function means
    /// the walk is decoding data, such as padding after a call that never returns, and it stops.
    fn branch(
        &mut self,
        function: &mut FunctionWalk,
        target: u64,
        conditional: bool,
        tail_call: bool,
        tracker: &Tracker,
    ) -> bool {
        let leaves = !function.range.contains(&target);
        if leaves && conditional {
            return false;
        }
        if leaves || tail_call {
            self.entry(target, function.mode);
        } else {
            function.blocks.insert(target, tracker);
        }
        conditional
    }

    /// Applies `instruction` to the registers, marking the literal it loads as data and taking a
    /// pointer it computes as a function.
    fn track(&mut self, tracker: &mut Tracker, pc: u64, instruction: &Instruction) {
        if let Effect::Literal { address, .. } = instruction.effect {
            self.take_data(address, 4);
        }
        let computed = tracker.apply(self.image, instruction.effect);
        if let Some(Computed::Pointer(address)) = computed {
            self.pointer(address);
        }
        if let Some(
            Computed::Pointer(address) | Computed::Offset(address) | Computed::Read(address),
        ) = computed
            && self.data.contains(address)
        {
            self.found.push((pc, address));
        }
    }

    /// Marks data the code reads, so no walk decodes it.
    fn take_data(&mut self, address: u64, size: u64) {
        if let Some(section) = self.section(address)
            && self.sections[section].is_free(address & !1, size)
        {
            self.sections[section].take(address, size);
        }
    }
}
