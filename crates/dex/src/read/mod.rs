// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

pub(crate) mod annotation;
mod bytes;
pub(crate) mod class;
pub(crate) mod code;
pub(crate) mod debug;
pub(crate) mod encoded_value;
pub(crate) mod header;
pub(crate) mod hidden_api;
pub(crate) mod ids;
pub mod parse;

pub(crate) use bytes::{i32_at, read_u8, read_u16, read_u32, u16_at, u32_at};
pub use parse::parse;
pub use parse::parse_bytes;
pub use parse::parse_container;
pub use parse::parse_owned;
use tracing::debug;

/// Parse a DEX file from a filesystem path.
///
/// The resulting [`crate::DexFile`] holds a memory map of the file directly,
/// so no extra heap copy of the file is made.
///
/// # Examples
///
/// ```text
/// use reseam_dex::{parse_file, ParseOptions};
///
/// let dex = parse_file("classes.dex", ParseOptions::default())?;
/// assert!(!dex.strings().is_empty());
/// ```
pub fn parse_file(
    path: impl AsRef<std::path::Path>,
    opts: crate::types::header::ParseOptions,
) -> crate::error::Result<crate::file::DexFile> {
    use std::sync::Arc;

    let path = path.as_ref();
    debug!(path = %path.display(), classes = ?opts.classes, "parsing DEX file from disk");

    let file = std::fs::File::open(path).map_err(crate::error::DexError::Io)?;
    // SAFETY: The caller must not mutate the file while the mapping is live.
    let mmap = unsafe { reseam_storage::map_file(&file) }.map_err(crate::error::DexError::Io)?;
    parse_bytes(crate::file::DexBytes::from_mmap(Arc::new(mmap)), opts)
}

/// Decodes `insns_size` DEX code units starting at `start` in a byte buffer.
/// Truncated or malformed encodings fail; unknown opcodes retain all their
/// original code units. Pool indices are resolved by the owning DEX file.
pub use code::decode_instructions;
