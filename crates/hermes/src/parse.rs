// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::bytes::{align, read_u32, slice};
use crate::error::{HermesError, Result, invalid};
use crate::file::HermesFile;
use crate::function::FunctionId;
use crate::layout::{FOOTER_SIZE, Field, HEADER_SIZE, MAGIC, Section};
use crate::opcode::BytecodeVersion;
use crate::string::StringId;

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
            if run.kind == crate::string::StringKind::Identifier {
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
