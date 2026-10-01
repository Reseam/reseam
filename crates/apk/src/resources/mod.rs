// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

mod attribute;
mod bags;
mod complex;
mod config;
mod entry;
mod package;
mod reader;
mod res_type;
mod scope;
mod type_spec;
mod values;
mod writer;

mod table;

pub use table::*;

use std::borrow::Cow;
use std::ops::Range;

use reseam_storage::Bytes;

use crate::error::{Result, invalid};
use crate::string_pool::StringPool;
use crate::value::ResValue;

pub use attribute::AttrFormats;
pub use config::config_for_qualifiers;
pub use entry::{EntryValue, MapEntry, ResEntry};
pub use package::ResPackage;
pub use res_type::ResType;
pub use scope::{ResourceScope, SplitLookup};
pub use type_spec::TypeSpec;
