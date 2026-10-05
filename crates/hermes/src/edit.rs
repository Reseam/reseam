use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use crate::error::{Result, invalid};
use crate::model::{FunctionHeader, Section};
use crate::{FunctionId, HermesFile, StringId, StringKind};

#[derive(Clone)]
pub(crate) struct EditedFunction {
    pub header: FunctionHeader,
    pub body: FunctionBody,
    pub exceptions: Vec<[u32; 3]>,
}

#[derive(Clone)]
pub(crate) enum FunctionBody {
    Original(Range<usize>),
    Owned(Vec<u8>),
}

impl FunctionBody {
    pub(crate) fn bytes<'a>(&'a self, file: &'a HermesFile<'_>) -> &'a [u8] {
        match self {
            Self::Original(range) => &file.source[range.clone()],
            Self::Owned(bytes) => bytes,
        }
    }
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Original(range) => range.len(),
            Self::Owned(bytes) => bytes.len(),
        }
    }
    pub(crate) fn place(&self, cursor: usize) -> usize {
        let remainder = match self {
            Self::Original(range) => range.start & 3,
            Self::Owned(_) => 0,
        };
        cursor + ((remainder + 4 - (cursor & 3)) & 3)
    }
}

#[derive(Clone)]
pub(crate) struct Hook {
    pub module: crate::ModuleId,
    pub export: StringId,
    pub bound: Vec<crate::wrap::Bound>,
}

/// Owns edits over a borrowed file. Original function and string identities
/// remain stable. Only additions, changed bodies and indices allocate storage.
/// Indices and closure analysis are built lazily and retained in detached edits.
pub struct Editor<'a> {
    pub(crate) file: HermesFile<'a>,
    pub(crate) edits: Edits,
}

/// Detached edits for a long-lived owner of the input mapping. Resume them only
/// against the exact original file. No source data is copied into this state.
pub struct Edits {
    pub(crate) base_hash: [u8; 20],
    pub(crate) additions: [Vec<u8>; 15],
    pub(crate) functions: BTreeMap<FunctionId, EditedFunction>,
    pub(crate) appended: Vec<EditedFunction>,
    pub(crate) strings: Vec<AddedString>,
    pub(crate) string_index: rustc_hash::FxHashMap<(u64, StringKind), Vec<StringId>>,
    original_strings_indexed: bool,
    pub(crate) global: FunctionId,
    pub(crate) modules: Vec<FunctionId>,
    pub(crate) module_exports: Vec<Vec<StringId>>,
    pub(crate) string_switches: u32,
    pub(crate) wrapped: BTreeSet<FunctionId>,
    pub(crate) hook_graph: Option<crate::wrap::HookGraph>,
    pub(crate) roots: BTreeMap<FunctionId, crate::wrap::RootAttachment>,
    /// Wrapped app functions, each with the appended function now holding its original body.
    pub(crate) relocated: BTreeMap<FunctionId, FunctionId>,
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
        let base_hash = file.source[file.header[1] as usize - 20..file.header[1] as usize]
            .try_into()
            .expect("validated footer length");
        Self {
            file,
            edits: Edits {
                base_hash,
                additions: std::array::from_fn(|_| Vec::new()),
                functions: BTreeMap::new(),
                appended: Vec::new(),
                strings: Vec::new(),
                string_index: rustc_hash::FxHashMap::default(),
                original_strings_indexed: false,
                global,
                modules: Vec::new(),
                module_exports: Vec::new(),
                string_switches,
                wrapped: BTreeSet::new(),
                hook_graph: None,
                roots: BTreeMap::new(),
                relocated: BTreeMap::new(),
            },
        }
    }

    pub fn file(&self) -> &HermesFile<'a> {
        &self.file
    }

    /// Detaches owned edits so a caller can retain them beside its mapping
    /// without building a self-referential object. Use `resume` to continue.
    pub fn into_edits(self) -> Edits {
        self.edits
    }

    /// Restores detached state against the original file. A different footer
    /// is an error; callers must keep the underlying mapping unchanged.
    pub fn resume(file: HermesFile<'a>, edits: Edits) -> Result<Self> {
        if file.source[file.header[1] as usize - 20..file.header[1] as usize] != edits.base_hash {
            return Err(invalid(0, "edits belong to another source file"));
        }
        Ok(Self { file, edits })
    }

    // Fallible work may append data and populate immutable analysis caches.
    // Replacements and root attachments are published only after it succeeds.
    pub(crate) fn transaction<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let lengths = self.edits.additions.each_ref().map(Vec::len);
        let appended = self.edits.appended.len();
        let strings = self.edits.strings.len();
        let modules = self.edits.modules.len();
        let global = self.edits.global;
        let switches = self.edits.string_switches;
        let result = operation(self);
        if result.is_err() {
            for (bytes, length) in self.edits.additions.iter_mut().zip(lengths) {
                bytes.truncate(length);
            }
            self.edits.appended.truncate(appended);
            for string in &self.edits.strings[strings..] {
                let value = if string.utf16 {
                    crate::StringValue::Utf16(&string.value)
                } else {
                    crate::StringValue::Latin1(&string.value)
                };
                let key = (value.fingerprint(), string.kind);
                let ids = self
                    .edits
                    .string_index
                    .get_mut(&key)
                    .expect("appended strings are indexed");
                ids.retain(|id| *id != string.id);
                if ids.is_empty() {
                    self.edits.string_index.remove(&key);
                }
            }
            self.edits.strings.truncate(strings);
            self.edits.modules.truncate(modules);
            self.edits.module_exports.truncate(modules);
            self.edits.global = global;
            self.edits.string_switches = switches;
        }
        result
    }

    /// Reuses a string of the requested kind or appends one. Identifiers carry
    /// the v98 hash, and strings requiring UTF-16 or overflow entries use them.
    /// A string previously used only as a value remains a separate entry when
    /// subsequently interned as an identifier, preserving original IDs.
    /// The original string index is built once; later calls inspect only matching
    /// fingerprints and verify the text, returning the first matching ID.
    pub fn intern(&mut self, text: &str, kind: StringKind) -> Result<StringId> {
        self.index_original_strings()?;
        let units: Vec<_> = text.encode_utf16().collect();
        let utf16 = units.iter().any(|&unit| unit > 127);
        let value: Vec<_> = if utf16 {
            units.iter().flat_map(|u| u.to_le_bytes()).collect()
        } else {
            units.iter().map(|&u| u as u8).collect()
        };
        let key = if utf16 {
            crate::StringValue::Utf16(&value)
        } else {
            crate::StringValue::Latin1(&value)
        }
        .fingerprint();
        for id in self
            .edits
            .string_index
            .get(&(key, kind))
            .into_iter()
            .flatten()
        {
            if id.0 < self.file.string_count() {
                if self.file.string(*id)?.equals(text) {
                    return Ok(*id);
                }
                continue;
            }
            let string = &self.edits.strings[(id.0 - self.file.string_count()) as usize];
            let value = if string.utf16 {
                crate::StringValue::Utf16(&string.value)
            } else {
                crate::StringValue::Latin1(&string.value)
            };
            if string.kind == kind && value.equals(text) {
                return Ok(string.id);
            }
        }
        self.append_string(value, utf16, units.len() as u32, kind)
    }

    fn index_original_strings(&mut self) -> Result<()> {
        if self.edits.original_strings_indexed {
            return Ok(());
        }
        let mut index = rustc_hash::FxHashMap::<_, Vec<_>>::default();
        let mut id = 0;
        for entry in self.file.section(Section::Kinds).as_chunks::<4>().0 {
            let run = u32::from_le_bytes(*entry);
            let kind = if run >> 31 == 0 {
                StringKind::String
            } else {
                StringKind::Identifier
            };
            let end = id + (run & 0x7fff_ffff);
            for string in (id..end).map(StringId) {
                index
                    .entry((self.file.string(string)?.fingerprint(), kind))
                    .or_default()
                    .push(string);
            }
            id = end;
        }
        for (key, ids) in &self.edits.string_index {
            index.entry(*key).or_default().extend_from_slice(ids);
        }
        self.edits.string_index = index;
        self.edits.original_strings_indexed = true;
        Ok(())
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
                .checked_add(self.edits.strings.len() as u32)
                .ok_or_else(|| invalid(0, "too many strings"))?,
        );
        let storage = &mut self.edits.additions[Section::Storage as usize];
        if utf16 && (self.file.header[8] as usize + storage.len()) & 1 != 0 {
            storage.push(0);
        }
        let offset = self.file.header[8] as usize + storage.len();
        let offset =
            u32::try_from(offset).map_err(|_| invalid(0, "string storage exceeds four GiB"))?;
        let entry = if offset < 1 << 23 && length < 255 {
            offset << 1 | length << 24 | u32::from(utf16)
        } else {
            let overflow = self.file.header[7] as usize
                + self.edits.additions[Section::Overflow as usize].len() / 8;
            if overflow >= 1 << 23 {
                return Err(invalid(0, "overflow string table exceeds 23 bits"));
            }
            let table = &mut self.edits.additions[Section::Overflow as usize];
            table.extend_from_slice(&offset.to_le_bytes());
            table.extend_from_slice(&length.to_le_bytes());
            (overflow as u32) << 1 | 255 << 24 | u32::from(utf16)
        };
        self.edits.additions[Section::Strings as usize].extend_from_slice(&entry.to_le_bytes());
        let run = 1 | if kind == StringKind::Identifier {
            1 << 31
        } else {
            0
        };
        self.edits.additions[Section::Kinds as usize].extend_from_slice(&u32::to_le_bytes(run));
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
            self.edits.additions[Section::Hashes as usize].extend_from_slice(&hash.to_le_bytes());
        }
        self.edits.additions[Section::Storage as usize].extend_from_slice(&value);
        let key = if utf16 {
            crate::StringValue::Utf16(&value)
        } else {
            crate::StringValue::Latin1(&value)
        }
        .fingerprint();
        self.edits
            .string_index
            .entry((key, kind))
            .or_default()
            .push(id);
        self.edits.strings.push(AddedString {
            value,
            utf16,
            kind,
            id,
        });
        Ok(id)
    }

    pub(crate) fn next_function(&self) -> FunctionId {
        FunctionId(self.file.function_count() + self.edits.appended.len() as u32)
    }

    pub(crate) fn append_function(&mut self, function: EditedFunction) -> FunctionId {
        let id = self.next_function();
        self.edits.appended.push(function);
        id
    }
}
