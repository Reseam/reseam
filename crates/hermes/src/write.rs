// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;
use std::io::Write;

use sha1::Digest;

use crate::FunctionId;
use crate::HermesFile;
use crate::bytes::align;
use crate::edit::{EditedFunction, Edits};
use crate::error::{Result, invalid};
use crate::function::ExceptionHandler;
use crate::hook::rooted_functions;
use crate::layout::{FOOTER_SIZE, Field, Section};
use crate::link::bootstrap;

mod layout;

use layout::Layout;

/// Streams `file` with `edits`: untouched sections, bodies and metadata are copied from the
/// source, function headers and debug information are relocated, and the SHA-1 footer is
/// recomputed. Functions that carry the linked modules' environment and the bootstrap are
/// assembled here, once every edit is known.
pub(crate) fn write(file: &HermesFile<'_>, edits: &Edits, output: &mut impl Write) -> Result<()> {
    if edits.is_empty() {
        output.write_all(file.source)?;
        return Ok(());
    }
    let mut assembled = rooted_functions(file, edits)?;
    if let Some(bootstrap) = bootstrap(file, edits) {
        assembled.insert(edits.global(file), bootstrap);
    }
    let functions = functions(file, edits, &assembled);
    let layout = Layout::plan(file, edits, &functions)?;
    let mut sink = Sink {
        output,
        hash: sha1::Sha1::new(),
        position: 0,
    };
    sink.bytes(&layout.header)?;
    for function in &layout.functions {
        sink.bytes(&function.header.encode_small(function.large))?;
    }
    for section in &Section::ALL[1..] {
        sink.align()?;
        sink.bytes(file.section(*section))?;
        sink.bytes(edits.added(*section))?;
    }
    metadata(file, &layout, &mut sink)?;
    for edited in functions.iter().flatten() {
        sink.pad_to(edited.body.place(sink.position))?;
        sink.bytes(edited.body.bytes(file))?;
    }
    for (function, edited) in layout.functions.iter().zip(&functions) {
        if !function.emits_large {
            continue;
        }
        sink.align()?;
        sink.bytes(&function.header.encode_large())?;
        sink.align()?;
        if let Some(edited) = edited
            && !edited.exceptions.is_empty()
        {
            sink.bytes(&ExceptionHandler::encode_table(&edited.exceptions))?;
        }
    }
    let debug = file.field(Field::DebugInfoOffset) as usize;
    let length = file.field(Field::FileLength) as usize;
    sink.align()?;
    sink.bytes(&file.source[debug..length - FOOTER_SIZE])?;
    let digest = sink.hash.finalize();
    output.write_all(&digest)?;
    output.write_all(&file.source[length..])?;
    Ok(())
}

/// Each function's edited form by ID, or `None` where it is unchanged.
fn functions<'a>(
    file: &HermesFile<'_>,
    edits: &'a Edits,
    assembled: &'a BTreeMap<FunctionId, EditedFunction>,
) -> Vec<Option<&'a EditedFunction>> {
    (0..edits.function_count(file))
        .map(FunctionId)
        .map(|id| assembled.get(&id).or_else(|| edits.function(file, id)))
        .collect()
}

/// Copies the metadata between the sections and the debug information, with each original
/// large header pointing at its function's relocated body.
fn metadata(file: &HermesFile<'_>, layout: &Layout, sink: &mut Sink<'_, impl Write>) -> Result<()> {
    let mut start = file.sections[Section::Sources as usize].end;
    let end = file.field(Field::DebugInfoOffset) as usize;
    for (&offset, bytes) in &layout.patches {
        if offset < start || offset + 4 > end {
            return Err(invalid(offset, "large header outside metadata region"));
        }
        sink.bytes(&file.source[start..offset])?;
        sink.bytes(bytes)?;
        start = offset + 4;
    }
    sink.bytes(&file.source[start..end])
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
