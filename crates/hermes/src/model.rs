use std::hash::Hasher;
use std::ops::Range;

use crate::error::{Result, invalid};
use crate::parse::{read_u32, slice};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunctionId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StringId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StringKind {
    String,
    Identifier,
}

/// Borrowed string storage. UTF-16 may contain unpaired surrogates, so the raw
/// code units remain available even when conversion to Rust text fails.
#[derive(Debug, Clone, Copy)]
pub enum StringValue<'a> {
    Latin1(&'a [u8]),
    Utf16(&'a [u8]),
}

impl StringValue<'_> {
    pub(crate) fn fingerprint(self) -> u64 {
        let mut hash = rustc_hash::FxHasher::default();
        match self {
            Self::Latin1(bytes) => {
                for &byte in bytes {
                    hash.write_u16(u16::from(byte));
                }
            }
            Self::Utf16(bytes) => {
                for unit in bytes.as_chunks::<2>().0 {
                    hash.write_u16(u16::from_le_bytes(*unit));
                }
            }
        }
        hash.finish()
    }
    /// Converts to Unicode text. Latin-1 strings always convert; unpaired
    /// UTF-16 surrogates return an error rather than being replaced.
    pub fn text(self) -> Result<String> {
        let text = match self {
            Self::Latin1(bytes) => bytes.iter().map(|&b| char::from(b)).collect(),
            Self::Utf16(bytes) => String::from_utf16(
                &bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|b| u16::from_le_bytes([b[0], b[1]]))
                    .collect::<Vec<_>>(),
            )
            .map_err(|_| invalid(0, "string has unpaired UTF-16 surrogates"))?,
        };
        Ok(text)
    }

    pub fn equals(self, text: &str) -> bool {
        match self {
            Self::Latin1(bytes) => bytes.iter().map(|&b| u16::from(b)).eq(text.encode_utf16()),
            Self::Utf16(bytes) => bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .eq(text.encode_utf16()),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FunctionHeader {
    pub offset: u32,
    pub parameters: u32,
    pub loop_depth: u32,
    pub size: u32,
    pub name: u32,
    pub number_regs: u32,
    pub non_pointer_regs: u32,
    pub frame_size: u32,
    pub read_cache: u8,
    pub write_cache: u8,
    pub object_cache: u8,
    pub private_cache: u8,
    pub flags: u8,
    pub info_offset: usize,
}

/// A borrowed function view. Parameter count excludes the implicit `this`.
pub struct Function<'a> {
    pub(crate) version: crate::opcode::BytecodeVersion,
    pub(crate) header: FunctionHeader,
    pub(crate) source: &'a [u8],
    pub id: FunctionId,
}

impl<'a> Function<'a> {
    pub fn name(&self) -> StringId {
        StringId(self.header.name)
    }
    pub fn parameter_count(&self) -> u32 {
        self.header.parameters.saturating_sub(1)
    }
    pub fn frame_size(&self) -> u32 {
        self.header.frame_size
    }
    pub fn body(&self) -> &'a [u8] {
        &self.source
            [self.header.offset as usize..self.header.offset as usize + self.header.size as usize]
    }
    pub fn instructions(&self) -> Result<Vec<crate::opcode::Instruction>> {
        crate::opcode::decode(self.version, self.body())
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(usize)]
pub(crate) enum Section {
    Functions,
    Kinds,
    Hashes,
    Strings,
    Overflow,
    Storage,
    Values,
    Keys,
    Shapes,
    BigInts,
    BigIntStorage,
    Regexps,
    RegexpStorage,
    Modules,
    Sources,
}

/// A validated, borrowed execution-form HBC file. Construction accepts only
/// version 98. Trailing epilogues and all original padding are preserved.
pub struct HermesFile<'a> {
    pub(crate) version: crate::opcode::BytecodeVersion,
    pub(crate) source: &'a [u8],
    pub(crate) header: [u32; 23],
    pub(crate) sections: [Range<usize>; 15],
}

impl<'a> HermesFile<'a> {
    pub fn version(&self) -> u32 {
        self.version as u32
    }
    pub fn bytecode_version(&self) -> crate::opcode::BytecodeVersion {
        self.version
    }
    pub fn function_count(&self) -> u32 {
        self.header[3]
    }
    pub fn string_count(&self) -> u32 {
        self.header[6]
    }
    pub fn global_function(&self) -> FunctionId {
        FunctionId(self.header[2])
    }

    pub(crate) fn section(&self, section: Section) -> &'a [u8] {
        &self.source[self.sections[section as usize].clone()]
    }

    pub fn function(&self, id: FunctionId) -> Result<Function<'a>> {
        if id.0 >= self.function_count() {
            return Err(invalid(0, "function ID out of range"));
        }
        let header = crate::parse::function_header(
            self.source,
            self.sections[Section::Functions as usize].start + id.0 as usize * 12,
        )?;
        slice(self.source, header.offset as usize, header.size as usize)?;
        if header.name >= self.string_count() {
            return Err(invalid(
                header.offset as usize,
                "function name ID out of range",
            ));
        }
        Ok(Function {
            version: self.version,
            header,
            source: self.source,
            id,
        })
    }

    pub fn string(&self, id: StringId) -> Result<StringValue<'a>> {
        if id.0 >= self.string_count() {
            return Err(invalid(0, "string ID out of range"));
        }
        let entry = read_u32(self.section(Section::Strings), id.0 as usize * 4)?;
        let mut offset = (entry >> 1) & 0x7f_ffff;
        let mut length = entry >> 24;
        if length == 255 {
            let overflow = self.section(Section::Overflow);
            let position = offset as usize * 8;
            offset = read_u32(overflow, position)?;
            length = read_u32(overflow, position + 4)?;
        }
        let utf16 = entry & 1 != 0;
        let data = slice(
            self.section(Section::Storage),
            offset as usize,
            (length as usize)
                .checked_mul(if utf16 { 2 } else { 1 })
                .ok_or_else(|| invalid(offset as usize, "string length overflow"))?,
        )?;
        Ok(if utf16 {
            StringValue::Utf16(data)
        } else {
            StringValue::Latin1(data)
        })
    }

    /// Finds a string without materializing string storage. Multiple identical
    /// strings return the first ID; absence is a valid result.
    pub fn find_string(&self, text: &str) -> Result<Option<StringId>> {
        for index in 0..self.string_count() {
            let id = StringId(index);
            if self.string(id)?.equals(text) {
                return Ok(Some(id));
            }
        }
        Ok(None)
    }

    /// Returns the unique function matching all supplied constraints. `strings`
    /// matches strings referenced by any instruction, including property names.
    /// Zero and multiple matches are errors. Parameters exclude `this`.
    pub fn find_function(
        &self,
        name: Option<&str>,
        strings: &[&str],
        parameters: Option<u32>,
    ) -> Result<FunctionId> {
        let mut matches = Vec::new();
        for index in 0..self.function_count() {
            let id = FunctionId(index);
            let function = self.function(id)?;
            if parameters.is_some_and(|p| p != function.parameter_count()) {
                continue;
            }
            if let Some(name) = name
                && !self.string(function.name())?.equals(name)
            {
                continue;
            }
            let found = if strings.is_empty() {
                true
            } else {
                let mut referenced = Vec::new();
                for instruction in function.instructions()? {
                    for (operand, value) in instruction
                        .definition()
                        .operands
                        .iter()
                        .zip(instruction.values.iter())
                    {
                        if operand.id == crate::opcode::IdKind::String {
                            referenced.push(self.string(StringId(*value as u32))?);
                        }
                    }
                    if let Some(table) = crate::parse::switch_table(&function, &instruction)?
                        && table.stride == 8
                    {
                        for entry in table.bytes.chunks_exact(table.stride) {
                            referenced.push(self.string(StringId(read_u32(entry, 0)?))?);
                        }
                    }
                }
                strings
                    .iter()
                    .all(|text| referenced.iter().any(|s| s.equals(text)))
            };
            if found {
                matches.push(id);
            }
        }
        if let [id] = matches[..] {
            Ok(id)
        } else {
            Err(crate::HermesError::Match {
                query: format!("name={name:?}, strings={strings:?}, parameters={parameters:?}"),
                count: matches.len(),
            })
        }
    }
}
