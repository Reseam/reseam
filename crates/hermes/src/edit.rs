use std::collections::BTreeMap;

use crate::error::{Result, invalid};
use crate::model::{FunctionHeader, Section};
use crate::{FunctionId, HermesFile, StringId, StringKind};

pub(crate) struct EditedFunction {
    pub header: FunctionHeader,
    pub body: Vec<u8>,
    pub exceptions: Vec<[u32; 3]>,
}

/// Owns edits over a borrowed file. Original function and string identities
/// remain stable. Only additions and changed bodies allocate storage.
pub struct Editor<'a> {
    pub(crate) file: HermesFile<'a>,
    pub(crate) additions: [Vec<u8>; 15],
    pub(crate) functions: BTreeMap<FunctionId, EditedFunction>,
    pub(crate) appended: Vec<EditedFunction>,
    pub(crate) strings: Vec<AddedString>,
    pub(crate) global: FunctionId,
    pub(crate) modules: Vec<FunctionId>,
    pub(crate) string_switches: u32,
}

pub(crate) struct AddedString {
    pub value: Vec<u8>,
    pub utf16: bool,
    pub kind: StringKind,
    pub id: StringId,
}

impl<'a> Editor<'a> {
    pub fn new(file: HermesFile<'a>) -> Self {
        let global = file.global_function();
        let string_switches = file.header[16];
        Self {
            file,
            additions: std::array::from_fn(|_| Vec::new()),
            functions: BTreeMap::new(),
            appended: Vec::new(),
            strings: Vec::new(),
            global,
            modules: Vec::new(),
            string_switches,
        }
    }

    pub fn file(&self) -> &HermesFile<'a> {
        &self.file
    }

    /// Reuses a string of the requested kind or appends one. Identifiers carry
    /// the v98 hash, and strings requiring UTF-16 or overflow entries use them.
    /// A string previously used only as a value remains a separate entry when
    /// subsequently interned as an identifier, preserving original IDs.
    pub fn intern(&mut self, text: &str, kind: StringKind) -> Result<StringId> {
        let mut index = 0;
        for entry in self.file.section(Section::Kinds).as_chunks::<4>().0 {
            let run = u32::from_le_bytes(*entry);
            let entry_kind = if run >> 31 == 0 {
                StringKind::String
            } else {
                StringKind::Identifier
            };
            let end = index + (run & 0x7fff_ffff);
            if entry_kind == kind {
                for string in index..end {
                    if self.file.string(StringId(string))?.equals(text) {
                        return Ok(StringId(string));
                    }
                }
            }
            index = end;
        }
        for string in &self.strings {
            let value = if string.utf16 {
                crate::StringValue::Utf16(&string.value)
            } else {
                crate::StringValue::Latin1(&string.value)
            };
            if string.kind == kind && value.equals(text) {
                return Ok(string.id);
            }
        }
        let units: Vec<_> = text.encode_utf16().collect();
        let utf16 = units.iter().any(|&unit| unit > 255);
        let value = if utf16 {
            units.iter().flat_map(|u| u.to_le_bytes()).collect()
        } else {
            units.iter().map(|&u| u as u8).collect()
        };
        self.append_string(value, utf16, units.len() as u32, kind)
    }

    pub(crate) fn append_string(
        &mut self,
        value: Vec<u8>,
        utf16: bool,
        length: u32,
        kind: StringKind,
    ) -> Result<StringId> {
        let id = StringId(
            self.file
                .string_count()
                .checked_add(self.strings.len() as u32)
                .ok_or_else(|| invalid(0, "too many strings"))?,
        );
        let storage = &mut self.additions[Section::Storage as usize];
        if utf16 && (self.file.header[8] as usize + storage.len()) & 1 != 0 {
            storage.push(0);
        }
        let offset = self.file.header[8] as usize + storage.len();
        let offset =
            u32::try_from(offset).map_err(|_| invalid(0, "string storage exceeds four GiB"))?;
        let entry = if offset < 1 << 23 && length < 255 {
            offset << 1 | length << 24 | u32::from(utf16)
        } else {
            let overflow =
                self.file.header[7] as usize + self.additions[Section::Overflow as usize].len() / 8;
            if overflow >= 1 << 23 {
                return Err(invalid(0, "overflow string table exceeds 23 bits"));
            }
            let table = &mut self.additions[Section::Overflow as usize];
            table.extend_from_slice(&offset.to_le_bytes());
            table.extend_from_slice(&length.to_le_bytes());
            (overflow as u32) << 1 | 255 << 24 | u32::from(utf16)
        };
        self.additions[Section::Strings as usize].extend_from_slice(&entry.to_le_bytes());
        let run = 1 | if kind == StringKind::Identifier {
            1 << 31
        } else {
            0
        };
        self.additions[Section::Kinds as usize].extend_from_slice(&u32::to_le_bytes(run));
        if kind == StringKind::Identifier {
            let units: Vec<_> = if utf16 {
                value
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|b| u16::from_le_bytes([b[0], b[1]]))
                    .collect()
            } else {
                value.iter().map(|&b| u16::from(b)).collect()
            };
            let hash = units.iter().fold(0_u32, |hash, &unit| {
                let hash = hash.wrapping_add(u32::from(unit));
                let hash = hash.wrapping_add(hash << 10);
                hash ^ (hash >> 6)
            });
            self.additions[Section::Hashes as usize].extend_from_slice(&hash.to_le_bytes());
        }
        self.additions[Section::Storage as usize].extend_from_slice(&value);
        self.strings.push(AddedString {
            value,
            utf16,
            kind,
            id,
        });
        Ok(id)
    }

    pub(crate) fn next_function(&self) -> FunctionId {
        FunctionId(self.file.function_count() + self.appended.len() as u32)
    }

    pub(crate) fn append_function(&mut self, function: EditedFunction) -> FunctionId {
        let id = self.next_function();
        self.appended.push(function);
        id
    }
}
