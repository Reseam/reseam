// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io::Write;

use crate::Result;
use crate::edit::{EditedFunction, Editor};
use crate::error::invalid;
use crate::model::{
    ExceptionHandler, FOOTER_SIZE, Field, FunctionHeader, FunctionId, HEADER_SIZE,
    LARGE_HEADER_SIZE, SMALL_HEADER_SIZE, Section,
};
use crate::parse::align;

struct PlannedFunction {
    header: FunctionHeader,
    large_offset: Option<u32>,
    emit_large: bool,
}

impl Editor<'_> {
    /// Streams edited sections and bodies, relocates all function headers and
    /// debug information, and recomputes the SHA-1 footer. Untouched body and
    /// metadata regions are copied directly from the borrowed input. Original
    /// string/function IDs and trailing epilogues remain stable.
    /// Shared closure ancestors and the module initializer are assembled here,
    /// followed by one complete layout and relocation pass per write.
    #[expect(
        clippy::too_many_lines,
        reason = "the layout and streaming passes follow the same ordered file segments"
    )]
    pub fn write(&self, output: &mut impl Write) -> Result<()> {
        if self.edits.is_empty() {
            output.write_all(self.file.source)?;
            return Ok(());
        }
        let mut finalized = self.rooted_functions()?;
        if let Some(bootstrap) = self.bootstrap() {
            finalized.insert(self.edits.global, bootstrap);
        }
        let count = self.file.function_count() as usize + self.edits.appended.len();
        let old_tail = self.file.sections[Section::Sources as usize].end;
        let old_debug = self.file.field(Field::DebugInfoOffset) as usize;
        let mut cursor = HEADER_SIZE + count * SMALL_HEADER_SIZE;
        for section in &Section::ALL[1..] {
            cursor = align(cursor) + self.section_size(*section);
        }
        let new_tail = cursor;
        let shift = new_tail
            .checked_sub(old_tail)
            .ok_or_else(|| invalid(0, "section relocation underflow"))?;
        cursor += old_debug - old_tail;
        let mut planned = Vec::with_capacity(count);
        let mut patches = std::collections::BTreeMap::new();
        for index in 0..self.file.function_count() {
            let original = self.file.function(FunctionId(index))?;
            let mut header = original.header;
            let old_large = header.large;
            if let Some(offset) = old_large {
                patches.insert(
                    offset,
                    relocate(header.offset as usize, shift)?.to_le_bytes(),
                );
            }
            if let Some(edited) = self.edited_at(index as usize, &finalized) {
                header = edited.header.clone();
                cursor = edited.body.place(cursor);
                header.offset = as_offset(cursor)?;
                cursor += edited.body.len();
                planned.push(PlannedFunction {
                    header,
                    large_offset: None,
                    emit_large: true,
                });
            } else {
                header.offset = relocate(header.offset as usize, shift)?;
                let large_offset = old_large
                    .map(|offset| relocate(offset, shift))
                    .transpose()?;
                let emit_large = large_offset.is_none() && !header.fits_small();
                planned.push(PlannedFunction {
                    header,
                    large_offset,
                    emit_large,
                });
            }
        }
        for index in self.file.function_count() as usize..count {
            let edited = self
                .edited_at(index, &finalized)
                .expect("appended function exists");
            cursor = edited.body.place(cursor);
            let mut header = edited.header.clone();
            header.offset = as_offset(cursor)?;
            cursor += edited.body.len();
            planned.push(PlannedFunction {
                header,
                large_offset: None,
                emit_large: true,
            });
        }
        for (index, function) in planned.iter_mut().enumerate().filter(|(_, f)| f.emit_large) {
            cursor = align(cursor);
            function.large_offset = Some(as_offset(cursor)?);
            cursor = align(cursor + LARGE_HEADER_SIZE);
            if let Some(edited) = self.edited_at(index, &finalized)
                && !edited.exceptions.is_empty()
            {
                cursor += ExceptionHandler::table_size(edited.exceptions.len());
            }
        }
        cursor = align(cursor);
        let debug = as_offset(cursor)?;
        let file_length = self.file.field(Field::FileLength) as usize;
        let old_footer = file_length - FOOTER_SIZE;
        cursor += old_footer - old_debug;
        let mut header = self.file.source[..HEADER_SIZE].to_vec();
        let mut put = |field: Field, value: u32| {
            header[field as usize..field as usize + 4].copy_from_slice(&value.to_le_bytes());
        };
        put(Field::FileLength, as_offset(cursor + FOOTER_SIZE)?);
        put(Field::GlobalFunction, self.edits.global.0);
        put(Field::FunctionCount, count as u32);
        for section in &Section::ALL[1..] {
            put(section.count(), self.section_count(*section));
        }
        put(Field::DebugInfoOffset, debug);
        put(Field::StringSwitchCount, self.edits.string_switches);
        let mut sink = Sink {
            output,
            hash: ring::digest::Context::new(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY),
            position: 0,
        };
        sink.bytes(&header)?;
        for function in &planned {
            sink.bytes(&function.header.encode_small(function.large_offset))?;
        }
        for section in &Section::ALL[1..] {
            sink.align()?;
            sink.bytes(self.file.section(*section))?;
            sink.bytes(&self.edits.additions[*section as usize])?;
        }
        let mut start = old_tail;
        for (offset, bytes) in patches {
            if offset < start || offset + 4 > old_debug {
                return Err(invalid(offset, "large header outside metadata region"));
            }
            sink.bytes(&self.file.source[start..offset])?;
            sink.bytes(&bytes)?;
            start = offset + 4;
        }
        sink.bytes(&self.file.source[start..old_debug])?;
        for index in 0..count {
            if let Some(edited) = self.edited_at(index, &finalized) {
                sink.pad_to(edited.body.place(sink.position))?;
                sink.bytes(edited.body.bytes(&self.file))?;
            }
        }
        for (index, function) in planned.iter().enumerate().filter(|(_, f)| f.emit_large) {
            sink.align()?;
            sink.bytes(&function.header.encode_large())?;
            sink.align()?;
            if let Some(edited) = self.edited_at(index, &finalized)
                && !edited.exceptions.is_empty()
            {
                sink.bytes(&ExceptionHandler::encode_table(&edited.exceptions))?;
            }
        }
        sink.align()?;
        sink.bytes(&self.file.source[old_debug..old_footer])?;
        let digest = sink.hash.finish();
        output.write_all(digest.as_ref())?;
        output.write_all(&self.file.source[file_length..])?;
        Ok(())
    }

    fn edited_at<'a>(
        &'a self,
        index: usize,
        finalized: &'a std::collections::BTreeMap<FunctionId, EditedFunction>,
    ) -> Option<&'a EditedFunction> {
        if let Some(edited) = finalized.get(&FunctionId(index as u32)) {
            return Some(edited);
        }
        if index < self.file.function_count() as usize {
            self.edits.functions.get(&FunctionId(index as u32))
        } else {
            self.edits
                .appended
                .get(index - self.file.function_count() as usize)
        }
    }
}

fn as_offset(value: usize) -> Result<u32> {
    u32::try_from(value).map_err(|_| invalid(value, "file exceeds four GiB"))
}

fn relocate(offset: usize, shift: usize) -> Result<u32> {
    as_offset(
        offset
            .checked_add(shift)
            .ok_or_else(|| invalid(offset, "offset overflow"))?,
    )
}

struct Sink<'a, W> {
    output: &'a mut W,
    hash: ring::digest::Context,
    position: usize,
}

impl<W: Write> Sink<'_, W> {
    fn bytes(&mut self, bytes: &[u8]) -> Result<()> {
        self.output.write_all(bytes)?;
        self.hash.update(bytes);
        self.position += bytes.len();
        Ok(())
    }
    fn align(&mut self) -> Result<()> {
        self.pad_to(align(self.position))
    }
    fn pad_to(&mut self, position: usize) -> Result<()> {
        let count = position - self.position;
        self.bytes(&[0; 3][..count])
    }
}
