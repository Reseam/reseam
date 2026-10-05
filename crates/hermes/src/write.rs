use std::io::Write;

use crate::edit::Editor;
use crate::error::invalid;
use crate::model::{FunctionHeader, FunctionId, Section};
use crate::parse::align;
use crate::{HermesFile, Result};

impl HermesFile<'_> {
    /// Streams an unmodified file byte for byte, preserving padding, footer,
    /// debug information and trailing epilogue. IO errors propagate to callers.
    pub fn write(&self, output: &mut impl Write) -> Result<()> {
        output.write_all(self.source)?;
        Ok(())
    }
}

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
        if self.edits.strings.is_empty()
            && self.edits.appended.is_empty()
            && self.edits.functions.is_empty()
        {
            return self.file.write(output);
        }
        let mut finalized = self.rooted_functions()?;
        if let Some(bootstrap) = self.bootstrap() {
            finalized.insert(self.edits.global, bootstrap);
        }
        let count = self.file.function_count() as usize + self.edits.appended.len();
        let old_tail = self.file.sections[Section::Sources as usize].end;
        let old_debug = self.file.header[20] as usize;
        let mut cursor = 128 + count * 12;
        for section in 1..15 {
            cursor = align(cursor)
                + self.file.sections[section].len()
                + self.edits.additions[section].len();
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
            let old_large = (header.info_offset != 0).then(|| header.info_offset - 40);
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
                let emit_large = large_offset.is_none() && !fits_small(&header);
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
            cursor += 40;
            if let Some(edited) = self.edited_at(index, &finalized)
                && !edited.exceptions.is_empty()
            {
                cursor += 4 + edited.exceptions.len() * 12;
            }
        }
        cursor = align(cursor);
        let debug = as_offset(cursor)?;
        let old_footer = self.file.header[1] as usize - 20;
        cursor += old_footer - old_debug;
        let length = as_offset(cursor + 20)?;
        let mut header = self.file.source[..128].to_vec();
        put(&mut header, 32, length);
        put(&mut header, 36, self.edits.global.0);
        put(&mut header, 40, count as u32);
        let fields = [
            (Section::Kinds, 44, 4),
            (Section::Hashes, 48, 4),
            (Section::Strings, 52, 4),
            (Section::Overflow, 56, 8),
            (Section::Storage, 60, 1),
            (Section::BigInts, 64, 8),
            (Section::BigIntStorage, 68, 1),
            (Section::Regexps, 72, 8),
            (Section::RegexpStorage, 76, 1),
            (Section::Values, 80, 1),
            (Section::Keys, 84, 1),
            (Section::Shapes, 88, 8),
            (Section::Modules, 100, 8),
            (Section::Sources, 104, 8),
        ];
        for (section, offset, stride) in fields {
            put(
                &mut header,
                offset,
                ((self.file.section(section).len() + self.edits.additions[section as usize].len())
                    / stride) as u32,
            );
        }
        put(&mut header, 108, debug);
        put(&mut header, 92, self.edits.string_switches);
        let mut sink = Sink {
            output,
            hash: ring::digest::Context::new(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY),
            position: 0,
        };
        sink.bytes(&header)?;
        for function in &planned {
            sink.bytes(&small_header(&function.header, function.large_offset))?;
        }
        for section in 1..15 {
            sink.align()?;
            sink.bytes(&self.file.source[self.file.sections[section].clone()])?;
            sink.bytes(&self.edits.additions[section])?;
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
            sink.bytes(&large_header(&function.header))?;
            sink.align()?;
            if let Some(edited) = self.edited_at(index, &finalized)
                && !edited.exceptions.is_empty()
            {
                sink.bytes(&(edited.exceptions.len() as u32).to_le_bytes())?;
                for exception in &edited.exceptions {
                    for value in exception {
                        sink.bytes(&value.to_le_bytes())?;
                    }
                }
            }
        }
        sink.align()?;
        sink.bytes(&self.file.source[old_debug..old_footer])?;
        let digest = sink.hash.finish();
        output.write_all(digest.as_ref())?;
        output.write_all(&self.file.source[self.file.header[1] as usize..])?;
        Ok(())
    }

    fn edited_at<'a>(
        &'a self,
        index: usize,
        finalized: &'a std::collections::BTreeMap<FunctionId, crate::edit::EditedFunction>,
    ) -> Option<&'a crate::edit::EditedFunction> {
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

fn put(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn fits_small(h: &FunctionHeader) -> bool {
    h.offset < 1 << 25
        && h.parameters < 32
        && h.loop_depth < 4
        && h.size < 1 << 14
        && h.name < 256
        && h.number_regs < 32
        && h.non_pointer_regs < 32
        && h.frame_size < 256
        && h.write_cache < 64
        && h.object_cache < 2
        && h.private_cache < 2
        && h.flags & 0x18 == 0
}

fn small_header(h: &FunctionHeader, large: Option<u32>) -> [u8; 12] {
    let mut bytes = [0; 12];
    if let Some(offset) = large {
        put(&mut bytes, 0, offset & 0x00ff_ffff);
        put(&mut bytes, 4, (offset >> 24) << 14);
        bytes[11] = 32;
    } else {
        put(
            &mut bytes,
            0,
            h.offset | h.parameters << 25 | h.loop_depth << 30,
        );
        put(
            &mut bytes,
            4,
            h.size | h.name << 14 | h.number_regs << 22 | h.non_pointer_regs << 27,
        );
        bytes[8] = h.frame_size as u8;
        bytes[9] = h.read_cache;
        bytes[10] = h.write_cache | h.object_cache << 6 | h.private_cache << 7;
        bytes[11] = h.flags & !32;
    }
    bytes
}

fn large_header(h: &FunctionHeader) -> [u8; 37] {
    let mut bytes = [0; 37];
    for (index, value) in [
        h.offset,
        h.parameters,
        h.loop_depth,
        h.size,
        h.name,
        h.number_regs,
        h.non_pointer_regs,
        h.frame_size,
    ]
    .into_iter()
    .enumerate()
    {
        put(&mut bytes, index * 4, value);
    }
    bytes[32..].copy_from_slice(&[
        h.read_cache,
        h.write_cache,
        h.object_cache,
        h.private_cache,
        h.flags | 32,
    ]);
    bytes
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
