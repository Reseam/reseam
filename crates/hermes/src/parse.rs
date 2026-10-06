// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::Function;
use crate::error::{HermesError, Result, invalid};
use crate::model::{
    FOOTER_SIZE, Field, FunctionId, HEADER_SIZE, HermesFile, MAGIC, Section, StringId,
};
use crate::opcode::{BytecodeVersion, Instruction, Op};

pub(crate) fn slice(bytes: &[u8], offset: usize, length: usize) -> Result<&[u8]> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| invalid(offset, "range overflow"))?;
    bytes
        .get(offset..end)
        .ok_or_else(|| invalid(offset, "truncated section"))
}

pub(crate) fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let data = slice(bytes, offset, 4)?;
    Ok(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
}

pub(crate) const fn align(offset: usize) -> usize {
    (offset + 3) & !3
}

impl<'a> HermesFile<'a> {
    /// Parses without copying file data. All structured sections, string
    /// ranges, function bodies, and exception tables are bounds checked.
    /// Unsupported versions and delta-form files return errors.
    pub fn parse(source: &'a [u8]) -> Result<Self> {
        if slice(source, 0, 8)? != MAGIC.to_le_bytes() {
            return Err(HermesError::Magic);
        }
        let version = BytecodeVersion::parse(read_u32(source, Field::Version as usize)?)?;
        slice(source, 0, HEADER_SIZE)?;
        let header = |field: Field| read_u32(source, field as usize);
        let length = header(Field::FileLength)? as usize;
        if length < HEADER_SIZE + FOOTER_SIZE || length > source.len() {
            return Err(invalid(Field::FileLength as usize, "invalid file length"));
        }
        let footer = length - FOOTER_SIZE;
        let functions = header(Field::FunctionCount)?;
        if functions == 0 || header(Field::GlobalFunction)? >= functions {
            return Err(invalid(
                Field::GlobalFunction as usize,
                "invalid global function",
            ));
        }
        let mut cursor = HEADER_SIZE;
        let mut sections = std::array::from_fn(|_| 0..0);
        for section in Section::ALL {
            cursor = align(cursor);
            let size = (header(section.count())? as usize)
                .checked_mul(section.stride())
                .ok_or_else(|| invalid(cursor, "section size overflow"))?;
            slice(&source[..footer], cursor, size)?;
            sections[section as usize] = cursor..cursor + size;
            cursor += size;
        }
        let debug = header(Field::DebugInfoOffset)? as usize;
        if debug < cursor || debug > footer {
            return Err(invalid(
                Field::DebugInfoOffset as usize,
                "invalid debug info offset",
            ));
        }
        let file = Self {
            version,
            source,
            sections,
        };
        let kinds = file.sections[Section::Kinds as usize].start;
        let mut count = 0_u64;
        let mut identifiers = 0_u64;
        for run in file.kind_runs() {
            if run.count == 0 {
                return Err(invalid(kinds, "empty string kind run"));
            }
            count += u64::from(run.count);
            if run.kind == crate::model::StringKind::Identifier {
                identifiers += u64::from(run.count);
            }
        }
        if count != u64::from(file.string_count())
            || identifiers != u64::from(file.field(Field::IdentifierCount))
        {
            return Err(invalid(kinds, "string kind counts disagree"));
        }
        for (table, storage) in [
            (Section::BigInts, Section::BigIntStorage),
            (Section::Regexps, Section::RegexpStorage),
        ] {
            for entry in file.section(table).as_chunks::<8>().0 {
                slice(
                    file.section(storage),
                    read_u32(entry, 0)? as usize,
                    read_u32(entry, 4)? as usize,
                )?;
            }
        }
        for entry in file.section(Section::Shapes).as_chunks::<8>().0 {
            slice(file.section(Section::Keys), read_u32(entry, 0)? as usize, 0)?;
        }
        for id in 0..file.string_count() {
            file.string(StringId(id))?;
        }
        for id in 0..functions {
            let function = file.function(FunctionId(id))?;
            if function.body_range().end > debug {
                return Err(invalid(
                    function.header.offset as usize,
                    "function overlaps debug section",
                ));
            }
            for handler in function.exception_handlers()? {
                if handler.start > handler.end
                    || handler.end > function.header.size
                    || handler.target >= function.header.size
                {
                    return Err(invalid(
                        function.header.offset as usize,
                        "exception handler outside function",
                    ));
                }
            }
        }
        Ok(file)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SwitchKind {
    Integer,
    String,
}

impl SwitchKind {
    const fn stride(self) -> usize {
        match self {
            Self::Integer => 4,
            Self::String => 8,
        }
    }
}

/// The jump table a switch instruction appends after its function's code.
pub(crate) struct SwitchTable<'a> {
    pub kind: SwitchKind,
    /// The operand holding the table's offset relative to the instruction.
    pub operand: usize,
    pub offset: usize,
    pub bytes: &'a [u8],
}

impl SwitchTable<'_> {
    /// Each case's string key, for string switches, and relative jump target.
    pub(crate) fn entries(&self) -> impl Iterator<Item = (Option<StringId>, i32)> {
        self.bytes.chunks_exact(self.kind.stride()).map(|entry| {
            let word = |at: usize| {
                u32::from_le_bytes(
                    entry[at..at + 4]
                        .try_into()
                        .expect("entries hold whole words"),
                )
            };
            match self.kind {
                SwitchKind::Integer => (None, word(0).cast_signed()),
                SwitchKind::String => (Some(StringId(word(0))), word(4).cast_signed()),
            }
        })
    }
}

pub(crate) fn switch_table<'a>(
    function: &Function<'a>,
    inst: &Instruction,
) -> Result<Option<SwitchTable<'a>>> {
    let (kind, operand, count) = match inst.op {
        Op::UIntSwitchImm => (
            SwitchKind::Integer,
            1,
            inst.values[4]
                .checked_sub(inst.values[3])
                .and_then(|n| n.checked_add(1)),
        ),
        Op::StringSwitchImm => (SwitchKind::String, 2, Some(inst.values[4])),
        _ => return Ok(None),
    };
    let at = inst
        .offset
        .ok_or_else(|| invalid(0, "generated switch has no table"))? as usize;
    let count = count.ok_or_else(|| invalid(at, "invalid switch range"))?;
    let offset = (function.header.offset as usize)
        .checked_add(at)
        .and_then(|n| n.checked_add(usize::try_from(inst.values[operand]).ok()?))
        .filter(|&n| n <= usize::MAX - 3)
        .map(align)
        .ok_or_else(|| invalid(at, "switch offset overflow"))?;
    let size = usize::try_from(count)
        .ok()
        .and_then(|n| n.checked_mul(kind.stride()))
        .ok_or_else(|| invalid(offset, "switch table overflow"))?;
    Ok(Some(SwitchTable {
        kind,
        operand,
        offset,
        bytes: slice(function.source, offset, size)?,
    }))
}
