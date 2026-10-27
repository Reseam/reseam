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

mod edit;
mod encoding;
mod error;
mod file;
mod instruction;
mod model;
mod options;
mod read;
mod references;
mod search;
mod write;

pub use edit::block::{
    BlockInsertion, BlockOperand, BlockRegister, BlockStep, FrameGrowth, insert_block,
};
pub use edit::body::IncomingBranches;
pub use edit::forms::move_register;
pub use edit::grow::grow_registers;
pub use edit::invoke::{
    ArgumentMove, Invocation, InvokeEncoding, InvokeForm, Receiver, argument_kinds, array_kinds,
};
pub use edit::liveness::{find_free_register, reaching_definitions};
pub use edit::relocate::{BranchDestination, InstructionExpansion, InstructionMap};
pub use encoding::mutf8::decode_mutf8;
pub use error::{DexError, Result};
pub use file::memory::{MaterializationStats, MemoryBreakdown, estimated_ir_bytes};
pub use file::{ClassTable, DexFile, HiddenApiData, IdTable, StringPool};
pub use instruction::codec::{decode_instructions, encode_instructions};
pub use instruction::operands::RegisterKind;
pub use instruction::{
    FillArrayPayloadData, Instruction, PackedSwitchData, RegList, SparseSwitchData,
};
pub use model::access_flags::AccessFlags;
pub use model::annotation::{
    AnnotationElement, AnnotationItem, AnnotationVisibility, AnnotationsDirectory,
};
pub use model::class::{ClassData, ClassDef, ClassHeader, EncodedField, EncodedMethod, MethodKind};
pub use model::code::{CatchHandler, CodeItem, TryItem, TypedCatch};
pub use model::debug::{DebugBytecode, DebugInfo};
pub use model::descriptor::{is_type_descriptor, parse_method_descriptor};
pub use model::encoded_value::{EncodedAnnotation, EncodedAnnotationElement, EncodedValue};
pub use model::header::{DexHeader, DexVersion};
pub use model::hidden_api::{ClassHiddenApiFlags, HiddenApiFlags};
pub use model::map::MapItem;
pub use model::metadata::Metadata;
pub use model::method_handle::{CallSiteItem, MethodHandle, MethodHandleMember, MethodHandleType};
pub use model::{
    CallSiteIdx, FieldId, FieldIdx, MethodHandleIdx, MethodId, MethodIdx, ProtoIdx, Prototype,
    StringIdx, TypeIdx,
};
pub use options::{
    LinkPolicy, Loading, MAX_POOL_SIZE, MetadataPolicy, ParseOptions, Validation, Verification,
    WriteOptions,
};
pub use read::class::{ClassSkeleton, MemberCounts, MethodHeader};
pub use read::parse;
pub use read::parse::parse_container_with_bytes;
pub use read::parse_bytes;
pub use read::parse_container;
pub use read::parse_file;
pub use read::parse_owned;
pub use reseam_storage::Bytes as DexBytes;
pub use search::index::{RefKey, RefQuery};
pub use search::inspect::{MethodSummary, summarize_resident};
pub use search::pattern::{InstructionPattern, OpcodeMatcher};
pub use search::view::{InstructionHit, InstructionSite, MethodHit, MethodView};
pub use write::{
    DexPart, Spooled, WriteLayout, split_to_fit, write_class, write_container_spooled,
    write_spooled,
};
