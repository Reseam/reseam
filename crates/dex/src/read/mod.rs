// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! DEX parsing entrypoints and low-level readers.

pub(crate) mod annotation;
pub(crate) mod class;
pub(crate) mod code;
pub(crate) mod debug;
pub(crate) mod encoded_value;
pub(crate) mod header;
pub(crate) mod ids;
pub mod parse;

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
/// ```no_run
/// use reseam_dex::{parse_file, ParseOptions};
///
/// let dex = parse_file("classes.dex", ParseOptions::default())?;
/// assert!(!dex.strings.is_empty());
/// # Ok::<(), reseam_dex::DexError>(())
/// ```
pub fn parse_file(
    path: impl AsRef<std::path::Path>,
    opts: crate::types::header::ParseOptions,
) -> crate::error::Result<crate::file::DexFile> {
    use std::sync::Arc;

    let path = path.as_ref();
    debug!(path = %path.display(), lazy = opts.lazy, "parsing DEX file from disk");

    let file = std::fs::File::open(path).map_err(crate::error::DexError::Io)?;
    // SAFETY: The caller must not mutate the file while the mapping is live.
    let mmap = unsafe { memmap2::Mmap::map(&file) }.map_err(crate::error::DexError::Io)?;
    parse::parse_bytes(crate::file::DexBytes::from_mmap(Arc::new(mmap)), opts)
}
