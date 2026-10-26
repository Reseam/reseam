// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `reseam-dex` parses, inspects, mutates, and writes Dalvik DEX files.
//!
//! Typical entrypoints are [`parse`], [`parse_file`], and [`write_spooled`].
//!
//! # Examples
//!
//! Parse and stream a rewritten file without buffering the complete DEX:
//! ```text
//! use reseam_dex::{parse_file, write_spooled, Loading, ParseOptions};
//!
//! let dex = parse_file("classes.dex", ParseOptions {
//!     classes: Loading::Deferred,
//!     ..ParseOptions::default()
//! })?;
//! let (rewritten, _) = write_spooled(&dex, None)?;
//! let mut output = std::fs::File::create("classes-rewritten.dex")?;
//! std::io::copy(&mut rewritten.reader(), &mut output)?;
//! ```

pub mod encoding;
pub mod error;
pub mod file;
pub mod read;
mod references;
pub mod types;
pub mod util;
pub mod write;

pub use encoding::mutf8::decode_mutf8;
pub use error::{DexError, Result};
pub use file::memory::{MaterializationStats, MemoryBreakdown, estimated_ir_bytes};
pub use file::{DexBytes, FileRecord, FileTable};
pub use file::{
    DexFile, HiddenApiData, InstructionHit, InstructionPattern, InstructionSite, MemberCounts,
    MethodHit, MethodSummary, MethodView, OpcodeMatcher, RefKey, RefQuery, summarize_resident,
};
pub use read::class::{ClassSkeleton, MethodHeader};
pub use read::decode_instructions;
pub use read::parse;
pub use read::parse::parse_container_with_bytes;
pub use read::parse_bytes;
pub use read::parse_container;
pub use read::parse_file;
pub use read::parse_owned;
pub use types::access_flags::AccessFlags;
pub use types::annotation::{
    AnnotationElement, AnnotationItem, AnnotationVisibility, AnnotationsDirectory,
};
pub use types::class::{ClassData, ClassDef, EncodedField, EncodedMethod, MethodKind};
pub use types::code::{CatchHandler, CodeItem, TryItem, TypedCatch};
pub use types::code_rewrite::{
    BranchDestination, InstructionExpansion, InstructionMap, branch_offset,
};
pub use types::debug::DebugInfo;
pub use types::encoded_value::{EncodedAnnotation, EncodedAnnotationElement, EncodedValue};
pub use types::header::{DexHeader, DexVersion, Loading, ParseOptions, Validation, Verification};
pub use types::hidden_api::{ClassHiddenApiFlags, HiddenApiFlags};
pub use types::instruction::{
    FillArrayPayloadData, Instruction, PackedSwitchData, RegList, SparseSwitchData,
};
pub use types::map::MapItem;
pub use types::method_handle::{
    CallSiteIdx, CallSiteItem, MethodHandle, MethodHandleIdx, MethodHandleMember, MethodHandleType,
};
pub use types::register_allocation::grow_registers;
pub use types::register_analysis::{
    find_contiguous_free_registers, find_free_register, find_free_registers, reaching_definitions,
};
pub use types::{FieldId, FieldIdx, MethodId, MethodIdx, ProtoIdx, Prototype, StringIdx, TypeIdx};
pub use util::descriptor::{is_type_descriptor, parse_method_descriptor};
pub use write::write_container_spooled;
pub use write::{
    DexPart, LinkPolicy, MetadataPolicy, Spooled, WriteLayout, WriteOptions, encode_instructions,
    split_to_fit, write_class, write_spooled,
};
