// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod find;

use std::io::Write;
use std::sync::OnceLock;

use reseam_bytes::u32_at;
use reseam_journal::{Journal, Savepoint};
use reseam_storage::Bytes;

use crate::arm::Mode;
use crate::code::Code;
use crate::error::{ElfError, Result};
use crate::file::ElfImage;
use crate::{Branch, Function, InstructionSet, a64, arm};

const CHUNK: usize = 1 << 20;

/// Finds and edits functions in a shared library. The library stays in its source storage,
/// which must not change while the editor lives; edits overlay it when it is written.
pub struct Editor {
    image: ElfImage,
    /// Where the functions lie, read on the first lookup that needs it.
    code: OnceLock<Code>,
    /// File offsets and the bytes written there, in the order they were made.
    edits: Vec<(usize, Vec<u8>)>,
    /// How many edits there were when each savepoint opened.
    journal: Journal<(), usize>,
}

impl Editor {
    /// Validates `source`.
    pub fn new(source: Bytes) -> Result<Self> {
        Ok(Self {
            image: ElfImage::parse(source)?,
            code: OnceLock::new(),
            edits: Vec::new(),
            journal: Journal::default(),
        })
    }

    /// Replaces `function`'s code with a return of `value` in the first result register: `w0`
    /// in AArch64, `r0` in 32-bit ARM. An AArch64 function keeps a leading landing pad of branch
    /// protection, so indirect calls into it remain valid, and authenticates a return address it
    /// signed before the return.
    pub fn always_return(&mut self, function: Function, value: i32) -> Result<()> {
        let (offset, code) = match function.instruction_set {
            InstructionSet::A64 => {
                let first = self.code_offset(function, 0, 4)?;
                a64::return_code(u32_at(self.image.bytes(), first), value)
            }
            InstructionSet::A32 => (0, arm::return_code(Mode::Arm, value)),
            InstructionSet::T32 => (0, arm::return_code(Mode::Thumb, value)),
        };
        let at = self.code_offset(function, offset, code.len() as u64)?;
        self.edit(at, code);
        Ok(())
    }

    /// Makes `branch` jump whatever its condition.
    pub fn always_take(&mut self, branch: Branch) -> Result<()> {
        let code = match branch.instruction_set {
            InstructionSet::A64 => a64::jump(branch.address, branch.target)
                .to_le_bytes()
                .to_vec(),
            InstructionSet::A32 => arm::jump(Mode::Arm, branch),
            InstructionSet::T32 => arm::jump(Mode::Thumb, branch),
        };
        self.replace_branch(branch, code)
    }

    /// Makes `branch` fall through whatever its condition.
    pub fn never_take(&mut self, branch: Branch) -> Result<()> {
        let code = match branch.instruction_set {
            InstructionSet::A64 => a64::NOP.to_le_bytes().to_vec(),
            InstructionSet::A32 => arm::nop(Mode::Arm, branch),
            InstructionSet::T32 => arm::nop(Mode::Thumb, branch),
        };
        self.replace_branch(branch, code)
    }

    /// Writes `code`, of the branch's size, over `branch`.
    fn replace_branch(&mut self, branch: Branch, code: Vec<u8>) -> Result<()> {
        let at = self.image.offset(branch.address, branch.size)?;
        self.edit(at, code);
        Ok(())
    }

    /// Writes `bytes` over `function`'s code at `offset` bytes from its start. An edit reaching
    /// past the function fails.
    pub fn replace_bytes(&mut self, function: Function, offset: u64, bytes: &[u8]) -> Result<()> {
        let at = self.code_offset(function, offset, bytes.len() as u64)?;
        self.edit(at, bytes.to_vec());
        Ok(())
    }

    /// The file offset of `len` bytes at `offset` into `function`'s code.
    fn code_offset(&self, function: Function, offset: u64, len: u64) -> Result<usize> {
        let needed = offset.saturating_add(len);
        if needed > function.size {
            return Err(ElfError::TooSmall { function, needed });
        }
        self.image
            .offset(function.address.saturating_add(offset), len)
    }

    fn edit(&mut self, offset: usize, bytes: Vec<u8>) {
        self.journal.record((), || self.edits.len());
        self.edits.push((offset, bytes));
    }

    /// How many edits the library holds. It only grows until a rollback.
    pub fn edits(&self) -> usize {
        self.edits.len()
    }

    /// Streams the library with every edit applied.
    pub fn write(&self, mut out: impl Write) -> Result<()> {
        let source = self.image.bytes();
        let mut buffer = Vec::with_capacity(CHUNK);
        for start in (0..source.len()).step_by(CHUNK) {
            let end = (start + CHUNK).min(source.len());
            buffer.clear();
            buffer.extend_from_slice(&source[start..end]);
            for (offset, bytes) in &self.edits {
                let from = (*offset).max(start);
                let to = (offset + bytes.len()).min(end);
                if from < to {
                    buffer[from - start..to - start]
                        .copy_from_slice(&bytes[from - offset..to - offset]);
                }
            }
            out.write_all(&buffer)?;
        }
        out.flush()?;
        Ok(())
    }

    /// Starts keeping the edits made from here, so [`Self::rollback_to`] can return to this point.
    pub fn open_savepoint(&mut self, savepoint: Savepoint) {
        self.journal.open(savepoint);
    }

    /// Keeps the edits made since `savepoint` as part of the savepoint around it.
    pub fn release_savepoint(&mut self, savepoint: Savepoint) {
        self.journal.release(savepoint);
    }

    /// Undoes every edit made since `savepoint` was opened and closes it, with every savepoint
    /// opened after it.
    pub fn rollback_to(&mut self, savepoint: Savepoint) {
        for length in self.journal.rollback(savepoint) {
            self.edits.truncate(length);
        }
    }
}
