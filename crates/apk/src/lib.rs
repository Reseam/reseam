pub mod apk_file;
pub mod axml;
pub(crate) mod buf;
pub mod dex;
pub mod error;
pub mod resources;
pub(crate) mod string_encoding;
pub mod zip;

pub use apk_file::{ApkComponent, ApkFile, ApkKind};
pub use axml::{AxmlAttribute, AxmlDocument, AxmlEvent, TypedValue};
pub use dex::{dex_to_entries, extract_dex, extract_dex_unified, from_apk};
pub use error::{ApkError, Result};
pub use resources::ResourceTable;
pub use zip::reader::ApkReader;
pub use zip::writer::ApkWriter;

pub use stitch_dex;
