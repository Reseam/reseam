// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `resources.arsc`: one model, read lazily from the file it was parsed from
//! and written back with only the edited chunks rebuilt.

mod attribute;
mod bags;
mod config;
mod entry;
mod id;
mod insert;
mod lookup;
mod package;
mod reader;
mod res_type;
mod scope;
mod staged_aliases;
mod strings;
mod type_spec;
mod writer;

use std::borrow::Cow;
use std::ops::Range;

use reseam_storage::Bytes;

use crate::error::Result;
use crate::string_pool::StringPool;

pub use attribute::{AttrFormats, AttrSymbol};
pub use config::config_for_qualifiers;
pub use entry::{EntryValue, MapEntry, ResEntry};
pub(crate) use lookup::first_found;
pub use package::ResPackage;
pub use res_type::ResType;
pub use scope::{ResourceScope, SplitLookup};
pub use strings::ResourceRef;
pub use type_spec::TypeSpec;

const RES_TABLE_TYPE: u16 = 0x0002;
const RES_TABLE_PACKAGE_TYPE: u16 = 0x0200;
const RES_TABLE_TYPE_SPEC: u16 = 0x0202;
const RES_TABLE_TYPE_TYPE: u16 = 0x0201;
const TABLE_HEADER_LEN: usize = 12;

#[derive(Debug, Clone)]
pub struct ResourceTable {
    global_strings: StringPool,
    packages: Vec<ResPackage>,
    data: Bytes,
    header: Range<usize>,
    suffix: Range<usize>,
    chunks: Vec<TableChunk>,
}

/// A top-level chunk in file order: a parsed part of the model, or bytes
/// kept as they are.
#[derive(Debug, Clone)]
enum TableChunk {
    Strings,
    Package(usize),
    Raw(Range<usize>),
}

impl ResourceTable {
    /// Creates an owned table. Parsed tables instead retain file-backed ranges
    /// for every chunk and header extension.
    pub fn new(global_strings: StringPool, packages: Vec<ResPackage>) -> Self {
        let chunks = std::iter::once(TableChunk::Strings)
            .chain((0..packages.len()).map(TableChunk::Package))
            .collect();
        Self {
            global_strings,
            packages,
            data: Bytes::default(),
            header: 0..0,
            suffix: 0..0,
            chunks,
        }
    }

    pub fn packages(&self) -> &[ResPackage] {
        &self.packages
    }

    pub fn get_string(&self, index: u32) -> Result<Option<Cow<'_, str>>> {
        self.global_strings.get(index)
    }

    pub fn set_string(&mut self, index: u32, value: String) -> Result<()> {
        self.global_strings.set(index, value)
    }

    /// Adds a string to the global pool and returns its index. Strings added
    /// earlier in this run are reused; the file's own strings are not
    /// searched, since that would index every translation in the table.
    pub fn add_global_string(&mut self, value: &str) -> u32 {
        self.global_strings.intern_added(value)
    }
}
