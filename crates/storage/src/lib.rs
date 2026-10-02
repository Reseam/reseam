// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

mod bytes;
pub mod file;
mod mapping;
mod scratch;

pub use bytes::Bytes;
pub use mapping::{MappedFile, map_file, map_range};
pub use scratch::ScratchDir;

pub use file::{temp_root, temporary_file};

mod path;
pub use path::canonicalize;
