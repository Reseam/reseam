// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `reseam-dex` parses, inspects, mutates, and writes Dalvik DEX files.
//!
//! Typical entrypoints are [`parse`], [`parse_file`], and [`write`].
//!
//! # Examples
//!
//! Parse a single DEX buffer:
//! ```no_run
//! use reseam_dex::{parse, ParseOptions};
//!
//! let bytes = std::fs::read("classes.dex")?;
//! let dex = parse(&bytes, ParseOptions::default())?;
//! println!("{} classes", dex.classes.len());
//! # Ok::<(), reseam_dex::DexError>(())
//! ```
//!
//! Parse and rewrite an on-disk DEX file:
//! ```no_run
//! use reseam_dex::{parse_file, write, ParseOptions};
//!
//! let mut dex = parse_file("classes.dex", ParseOptions::default())?;
//! let rewritten = write(&mut dex)?;
//! std::fs::write("classes-rewritten.dex", rewritten)?;
//! # Ok::<(), reseam_dex::DexError>(())
//! ```

pub mod encoding;
pub mod error;
pub mod file;
pub mod read;
pub mod types;
pub mod util;
pub mod write;

pub use error::{DexError, Result};
pub use file::container::{MaterializationStats, MemoryBreakdown, MultiDexContainer};
pub use file::{
    DexFile, Fingerprint, RefKey, RefQuery, FingerprintBuilder, FingerprintHit, InstructionHit, InstructionPattern,
    summarize_resident, InstructionSite, MemberCounts, MethodHit, MethodSummary, MethodView,
    OpcodeMatcher,
};
pub use read::class::{ClassSkeleton, MethodHeader};
pub use read::parse;
pub use read::parse_bytes;
pub use read::parse_container;
pub use read::parse_file;
pub use read::parse_owned;
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
pub use types::instruction::{
    FillArrayPayloadData, Instruction, PackedSwitchData, RegList, SparseSwitchData,
};
pub use types::label::{CodeBuilder, Label};
pub use types::map::MapItem;
pub use types::method_handle::{CallSiteIdx, CallSiteItem, MethodHandle, MethodHandleIdx};
pub use types::register_analysis::{
    find_contiguous_free_registers, find_free_register, find_free_registers,
};
pub use types::{
    FieldId, FieldIdx, MethodId, MethodIdx, ProtoIdx, Prototype, StringIdx, TypeIdx,
};
pub use write::write;
pub use write::write_container;
pub use write::{write_spooled, Spooled};
