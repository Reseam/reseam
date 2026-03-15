pub mod apk_file;
pub mod axml;
pub mod error;
pub mod multi_dex;
pub mod resources;
pub mod zip;

pub use apk_file::{ApkFile, ApkKind, ApkComponent};
pub use axml::reader::AxmlDocument;
pub use error::{ApkError, Result};
pub use multi_dex::{dex_to_entries, extract_dex, extract_dex_unified, from_apk};
pub use zip::reader::ApkReader;
pub use zip::writer::ApkWriter;

// Re-export stitch-dex so downstream crates don't need to depend on it directly
pub use stitch_dex;
