//! `stitch-dex` parses, inspects, mutates, and writes Dalvik DEX files.
//!
//! Typical entrypoints are [`parse`], [`parse_file`], and [`write`].
//!
//! # Examples
//!
//! Parse a single DEX buffer:
//! ```no_run
//! use stitch_dex::{parse, ParseOptions};
//!
//! let bytes = std::fs::read("classes.dex")?;
//! let dex = parse(&bytes, ParseOptions::default())?;
//! println!("{} classes", dex.classes.len());
//! # Ok::<(), stitch_dex::DexError>(())
//! ```
//!
//! Parse and rewrite an on-disk DEX file:
//! ```no_run
//! use stitch_dex::{parse_file, write, ParseOptions};
//!
//! let mut dex = parse_file("classes.dex", ParseOptions::default())?;
//! let rewritten = write(&mut dex)?;
//! std::fs::write("classes-rewritten.dex", rewritten)?;
//! # Ok::<(), stitch_dex::DexError>(())
//! ```

pub mod encoding;
pub mod error;
pub mod file;
pub mod read;
pub mod types;
pub mod util;
pub mod write;

pub use error::{DexError, Result};
pub use file::container::MultiDexContainer;
pub use file::{
    DexFile, Fingerprint, FingerprintBuilder, FingerprintMatch, InstructionPattern, MethodMatch,
    OpcodeMatcher,
};
pub use read::parse;
pub use read::parse_container;
pub use read::parse_file;
pub use types::access_flags::AccessFlags;
pub use types::annotation::{
    AnnotationElement, AnnotationItem, AnnotationVisibility, AnnotationsDirectory,
};
pub use types::class::{ClassData, ClassDef, EncodedField, EncodedMethod};
pub use types::code::{CatchHandler, CodeItem, TryItem, TypedCatch};
pub use types::debug::DebugInfo;
pub use types::encoded_value::EncodedValue;
pub use types::header::{DexHeader, DexVersion, ParseOptions};
pub use types::hidden_api::{ClassHiddenApiFlags, HiddenApiData, HiddenApiFlag};
pub use types::instruction::Instruction;
pub use types::label::{CodeBuilder, Label};
pub use types::map::MapItem;
pub use types::method_handle::{CallSiteIdx, CallSiteItem, MethodHandle, MethodHandleIdx};
pub use types::register_analysis::{find_free_register, find_free_registers};
pub use types::{
    DexString, FieldId, FieldIdx, MethodId, MethodIdx, ProtoIdx, Prototype, StringIdx, TypeIdx,
};
pub use write::write;
pub use write::write_container;
