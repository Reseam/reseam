// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use sha1::Digest;
use std::io::Write;

use crate::HermesFile;
use crate::Result;
use crate::bytes::align;
use crate::edit::{EditedFunction, Edits};
use crate::error::invalid;
use crate::function::{
    ExceptionHandler, FunctionHeader, FunctionId, LARGE_HEADER_SIZE, SMALL_HEADER_SIZE,
};
use crate::hook::rooted_functions;
use crate::layout::{FOOTER_SIZE, Field, HEADER_SIZE, Section};
use crate::link::bootstrap;

struct PlannedFunction {
    header: FunctionHeader,
    large_offset: Option<u32>,
    emit_large: bool,
}

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
pub(crate) fn write(file: &HermesFile<'_>, edits: &Edits, output: &mut impl Write) -> Result<()> {
    if edits.is_empty() {
        output.write_all(file.source)?;
        return Ok(());
    }
    let mut finalized = rooted_functions(file, edits)?;
    if let Some(bootstrap) = bootstrap(file, edits) {
        finalized.insert(edits.global(file), bootstrap);
    }
    let count = edits.function_count(file) as usize;
    let old_tail = file.sections[Section::Sources as usize].end;
    let old_debug = file.field(Field::DebugInfoOffset) as usize;
    let mut cursor = HEADER_SIZE + count * SMALL_HEADER_SIZE;
    for section in &Section::ALL[1..] {
        cursor = align(cursor) + edits.section_size(file, *section);
    }
    let new_tail = cursor;
    let shift = new_tail
        .checked_sub(old_tail)
        .ok_or_else(|| invalid(0, "section relocation underflow"))?;
    cursor += old_debug - old_tail;
    let mut planned = Vec::with_capacity(count);
    let mut patches = std::collections::BTreeMap::new();
    for index in 0..file.function_count() {
        let original = file.function(FunctionId(index))?;
        let mut header = original.header;
        let old_large = header.large;
        if let Some(offset) = old_large {
            patches.insert(
                offset,
                relocate(header.offset as usize, shift)?.to_le_bytes(),
            );
        }
        if let Some(edited) = edited_at(file, edits, index as usize, &finalized) {
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
    for index in file.function_count() as usize..count {
        let edited = edited_at(file, edits, index, &finalized).expect("appended function exists");
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
        if let Some(edited) = edited_at(file, edits, index, &finalized)
            && !edited.exceptions.is_empty()
        {
            cursor += ExceptionHandler::table_size(edited.exceptions.len());
        }
    }
    cursor = align(cursor);
    let debug = as_offset(cursor)?;
    let file_length = file.field(Field::FileLength) as usize;
    let old_footer = file_length - FOOTER_SIZE;
    cursor += old_footer - old_debug;
    let mut header = file.source[..HEADER_SIZE].to_vec();
    let mut put = |field: Field, value: u32| {
        header[field as usize..field as usize + 4].copy_from_slice(&value.to_le_bytes());
    };
    put(Field::FileLength, as_offset(cursor + FOOTER_SIZE)?);
    put(Field::GlobalFunction, edits.global(file).0);
    put(Field::FunctionCount, count as u32);
    for section in &Section::ALL[1..] {
        put(section.count(), edits.section_count(file, *section));
    }
    put(Field::DebugInfoOffset, debug);
    put(Field::StringSwitchCount, edits.string_switches());
    let mut sink = Sink {
        output,
        hash: sha1::Sha1::new(),
        position: 0,
    };
    sink.bytes(&header)?;
    for function in &planned {
        sink.bytes(&function.header.encode_small(function.large_offset))?;
    }
    for section in &Section::ALL[1..] {
        sink.align()?;
        sink.bytes(file.section(*section))?;
        sink.bytes(edits.added(*section))?;
    }
    let mut start = old_tail;
    for (offset, bytes) in patches {
        if offset < start || offset + 4 > old_debug {
            return Err(invalid(offset, "large header outside metadata region"));
        }
        sink.bytes(&file.source[start..offset])?;
        sink.bytes(&bytes)?;
        start = offset + 4;
    }
    sink.bytes(&file.source[start..old_debug])?;
    for index in 0..count {
        if let Some(edited) = edited_at(file, edits, index, &finalized) {
            sink.pad_to(edited.body.place(sink.position))?;
            sink.bytes(edited.body.bytes(file))?;
        }
    }
    for (index, function) in planned.iter().enumerate().filter(|(_, f)| f.emit_large) {
        sink.align()?;
        sink.bytes(&function.header.encode_large())?;
        sink.align()?;
        if let Some(edited) = edited_at(file, edits, index, &finalized)
            && !edited.exceptions.is_empty()
        {
            sink.bytes(&ExceptionHandler::encode_table(&edited.exceptions))?;
        }
    }
    sink.align()?;
    sink.bytes(&file.source[old_debug..old_footer])?;
    let digest = sink.hash.finalize();
    output.write_all(&digest)?;
    output.write_all(&file.source[file_length..])?;
    Ok(())
}

fn edited_at<'a>(
    file: &HermesFile<'_>,
    edits: &'a Edits,
    index: usize,
    finalized: &'a std::collections::BTreeMap<FunctionId, EditedFunction>,
) -> Option<&'a EditedFunction> {
    let id = FunctionId(index as u32);
    finalized.get(&id).or_else(|| edits.function(file, id))
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
    hash: sha1::Sha1,
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
