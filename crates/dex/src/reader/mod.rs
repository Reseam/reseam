//! DEX parsing entrypoints and low-level readers.

pub(crate) mod annotation_reader;
pub(crate) mod class_reader;
pub(crate) mod code_reader;
pub(crate) mod debug_reader;
pub(crate) mod encoded_value_reader;
pub(crate) mod header_reader;
pub(crate) mod id_reader;
pub mod parse;

pub use parse::parse;
pub use parse::parse_container;

/// Parse a DEX file from a filesystem path.
///
/// With the `mmap` feature enabled, this avoids a separate read buffer before
/// parsing. The parsed [`crate::DexFile`] still retains owned bytes for lazy access.
///
/// # Examples
///
/// ```no_run
/// use stitch_dex::{parse_file, ParseOptions};
///
/// let dex = parse_file("classes.dex", ParseOptions::default())?;
/// assert!(!dex.strings.is_empty());
/// # Ok::<(), stitch_dex::DexError>(())
/// ```
pub fn parse_file(
    path: impl AsRef<std::path::Path>,
    opts: crate::model::header::ParseOptions,
) -> crate::error::Result<crate::model::dex_file::DexFile> {
    let path = path.as_ref();

    #[cfg(feature = "mmap")]
    {
        let file = std::fs::File::open(path).map_err(crate::error::DexError::Io)?;
        // SAFETY: The caller must not mutate the file while the mapping is live.
        let mmap = unsafe { memmap2::Mmap::map(&file) }.map_err(crate::error::DexError::Io)?;
        parse::parse(&mmap, opts)
    }

    #[cfg(not(feature = "mmap"))]
    {
        let buf = std::fs::read(path).map_err(crate::error::DexError::Io)?;
        parse::parse(&buf, opts)
    }
}
