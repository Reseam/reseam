// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod bytes;
pub mod file;
mod hash;
mod mapping;
mod scratch;
mod spool;

pub use bytes::Bytes;
pub use hash::copy_hashed;
pub use mapping::{MappedFile, map_file, map_range};
pub use scratch::ScratchDir;
pub use spool::{copy_file, spool, spool_mapped};

pub use file::{temp_root, temporary_file};

mod path;
pub use path::canonicalize;
