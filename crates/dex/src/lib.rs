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
//! let dex = parse_file("classes.dex", ParseOptions::default())?;
//! let rewritten = write(&dex)?;
//! std::fs::write("classes-rewritten.dex", rewritten)?;
//! # Ok::<(), stitch_dex::DexError>(())
//! ```

pub mod encoding;
pub mod error;
pub mod model;
pub mod multi_dex;
pub mod reader;
pub mod util;
pub mod writer;

pub use error::{DexError, Result};
pub use model::access_flags::AccessFlags;
pub use model::annotation::{AnnotationItem, AnnotationVisibility, AnnotationsDirectory};
pub use model::call_site::{CallSiteIdx, CallSiteItem};
pub use model::class::{ClassData, ClassDef, EncodedField, EncodedMethod};
pub use model::code::CodeItem;
pub use model::debug::DebugInfo;
pub use model::dex_file::{DexFile, InstructionPattern, MethodMatch, OpcodeMatcher};
pub use model::encoded_value::EncodedValue;
pub use model::field::{FieldId, FieldIdx};
pub use model::header::{DexHeader, DexVersion, ParseOptions};
pub use model::hidden_api::{ClassHiddenApiFlags, HiddenApiData, HiddenApiFlag};
pub use model::instruction::Instruction;
pub use model::map::MapItem;
pub use model::method::{MethodId, MethodIdx};
pub use model::method_handle::{MethodHandle, MethodHandleIdx};
pub use model::proto::{ProtoIdx, Prototype};
pub use model::string::{DexString, StringIdx};
pub use model::types::TypeIdx;
pub use multi_dex::MultiDexContainer;
pub use reader::parse;
pub use reader::parse_file;
pub use writer::write;
