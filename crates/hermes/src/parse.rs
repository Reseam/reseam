// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::ops::Range;

use crate::FunctionId;
use crate::bytes::align;
use crate::error::{HermesError, Result, invalid};
use crate::file::HermesFile;
use crate::layout::{FOOTER_SIZE, Field, HEADER_SIZE, MAGIC, Section, VERSION};
use crate::string::{StringId, StringKind};
use reseam_bytes::{read_u32, require_len, slice, u32_at};

impl<'a> HermesFile<'a> {
    /// Parses without copying file data. All structured sections, string
    /// ranges, function bodies, and exception tables are bounds checked.
    /// Unsupported versions and delta-form files return errors.
    pub fn parse(source: &'a [u8]) -> Result<Self> {
        let file = Self {
            source,
            sections: sections(source)?,
        };
        file.check_strings()?;
        file.check_literals()?;
        file.check_functions()?;
        Ok(file)
    }

    fn check_strings(&self) -> Result<()> {
        let kinds = self.sections[Section::Kinds as usize].start;
        let (mut count, mut identifiers) = (0_u64, 0_u64);
        for run in self.kind_runs() {
            if run.count == 0 {
                return Err(invalid(kinds, "empty string kind run"));
            }
            count += u64::from(run.count);
            if run.kind == StringKind::Identifier {
                identifiers += u64::from(run.count);
            }
        }
        if count != u64::from(self.string_count())
            || identifiers != u64::from(self.field(Field::IdentifierCount))
        {
            return Err(invalid(kinds, "string kind counts disagree"));
        }
        for id in 0..self.string_count() {
            self.string(StringId(id))?;
        }
        Ok(())
    }

    /// Checks that bigints, regexps and object shapes refer into their storage.
    fn check_literals(&self) -> Result<()> {
        for (table, storage) in [
            (Section::BigInts, Section::BigIntStorage),
            (Section::Regexps, Section::RegexpStorage),
        ] {
            for entry in self.section(table).as_chunks::<8>().0 {
                self.section_bytes(
                    storage,
                    u32_at(entry, 0) as usize,
                    u32_at(entry, 4) as usize,
                )?;
            }
        }
        for entry in self.section(Section::Shapes).as_chunks::<8>().0 {
            self.section_bytes(Section::Keys, u32_at(entry, 0) as usize, 0)?;
        }
        Ok(())
    }

    fn check_functions(&self) -> Result<()> {
        let debug = self.field(Field::DebugInfoOffset);
        for id in 0..self.function_count() {
            let function = self.function(FunctionId(id))?;
            let header = &function.header;
            if function.body_range().end > debug as usize {
                return Err(invalid(
                    header.offset as usize,
                    "function overlaps debug section",
                ));
            }
            for handler in function.exception_handlers()? {
                if handler.start > handler.end
                    || handler.end > header.size
                    || handler.target >= header.size
                {
                    return Err(invalid(
                        header.offset as usize,
                        "exception handler outside function",
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Checks the header and finds each section, which must end before the debug information.
fn sections(source: &[u8]) -> Result<[Range<usize>; 15]> {
    if slice(source, 0, 8, "header")? != MAGIC.to_le_bytes() {
        return Err(HermesError::Magic);
    }
    let version = read_u32(source, Field::Version as usize, "header")?;
    if version != VERSION {
        return Err(HermesError::Version(version));
    }
    require_len(source, 0, HEADER_SIZE, "header")?;
    let header = |field: Field| u32_at(source, field as usize);
    let length = header(Field::FileLength) as usize;
    if length < HEADER_SIZE + FOOTER_SIZE || length > source.len() {
        return Err(invalid(Field::FileLength as usize, "invalid file length"));
    }
    let footer = length - FOOTER_SIZE;
    let functions = header(Field::FunctionCount);
    if functions == 0 || header(Field::GlobalFunction) >= functions {
        return Err(invalid(
            Field::GlobalFunction as usize,
            "invalid global function",
        ));
    }
    let mut cursor = HEADER_SIZE;
    let mut sections = std::array::from_fn(|_| 0..0);
    for section in Section::ALL {
        cursor = align(cursor);
        let size = (header(section.count()) as usize)
            .checked_mul(section.stride())
            .ok_or_else(|| invalid(cursor, "section size overflow"))?;
        require_len(&source[..footer], cursor, size, "Hermes section")?;
        sections[section as usize] = cursor..cursor + size;
        cursor += size;
    }
    let debug = header(Field::DebugInfoOffset) as usize;
    if debug < cursor || debug > footer {
        return Err(invalid(
            Field::DebugInfoOffset as usize,
            "invalid debug info offset",
        ));
    }
    Ok(sections)
}
