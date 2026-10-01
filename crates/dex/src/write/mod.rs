// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! DEX serialization entrypoints and helpers.

pub use self::part::{DexPart, split_to_fit};
pub use self::sink::{DexSink, SpoolSink, Spooled};
use crate::error::Result;
use crate::file::DexFile;
use crate::types::encoded_value::EncodedValue;
use crate::types::header::DexVersion;
use crate::types::map::MapItem;

pub(crate) mod annotations;
pub(crate) mod class_data;
pub(crate) mod code;
pub(crate) mod debug;
pub(crate) mod encoded_arrays;
pub(crate) mod encoded_value;
pub(crate) mod finalize;
pub(crate) mod instruction_writer;
pub(crate) mod intern;
pub(crate) mod orchestration;
pub(crate) mod part;
pub(crate) mod plan;
pub(crate) mod raw_code;
pub(crate) mod sink;
pub(crate) mod sort;

/// Selection of metadata to emit, independent of how it was loaded.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MetadataPolicy {
    #[default]
    Preserve,
    Omit,
    /// Omit metadata from this DEX's original mapping, retaining imported or authored metadata.
    OmitOriginal,
}

impl MetadataPolicy {
    pub(crate) fn keeps<T: crate::types::metadata::MetadataItem>(
        self,
        metadata: &crate::types::metadata::Metadata<T>,
        original: Option<&crate::file::DexBytes>,
    ) -> bool {
        match self {
            Self::Preserve => true,
            Self::Omit => false,
            Self::OmitOriginal => !metadata.belongs_to(original),
        }
    }
}

/// Opaque link bytes cannot be relocated after pool and section layout changes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LinkPolicy {
    #[default]
    Reject,
    Omit,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WriteOptions {
    pub debug_info: MetadataPolicy,
    pub annotations: MetadataPolicy,
    pub link_data: LinkPolicy,
}

pub(crate) fn is_default_value(v: &EncodedValue) -> bool {
    matches!(
        v,
        EncodedValue::Byte(0)
            | EncodedValue::Short(0)
            | EncodedValue::Char(0)
            | EncodedValue::Int(0)
            | EncodedValue::Long(0)
            | EncodedValue::Null
            | EncodedValue::Boolean(false)
    ) || matches!(v, EncodedValue::Float(f) if f.to_bits() == 0)
        || matches!(v, EncodedValue::Double(d) if d.to_bits() == 0)
}

/// Serializes a [`DexFile`] back into canonical DEX bytes.
///
/// Unedited classes remain file-backed. Code is remapped and emitted one method
/// at a time; the complete output is buffered by this convenience API. Use
/// [`write_spooled`] to stream large output. The source file is not modified.
pub fn write(dex: &DexFile) -> Result<Vec<u8>> {
    write_into(dex, None, Vec::new())
}

/// Serializes into an anonymous temp file instead of memory, optionally only
/// one [`DexPart`] of the file.
pub fn write_spooled(dex: &DexFile, part: Option<&DexPart>) -> Result<Spooled> {
    write_into(
        dex,
        part,
        SpoolSink::new().map_err(crate::error::DexError::Io)?,
    )?
    .finish()
}

fn write_into<S: DexSink>(dex: &DexFile, part: Option<&DexPart>, sink: S) -> Result<S> {
    let plan = plan::WritePlan::new(dex, part, dex.write_options())?;
    validate_index_limits(&plan)?;
    validate_link_policy(&plan)?;
    let mut w = DexWriter::new(sink);
    w.write_dex(&plan, dex.required_version())?;
    let span = MemberSpan {
        header: 0,
        end: w.pos(),
    };
    finalize::sign_member(&mut w, span)?;
    Ok(w.sink)
}

fn validate_link_policy(plan: &plan::WritePlan<'_>) -> Result<()> {
    if plan.dex.header.link_size != 0 && plan.options.link_data == LinkPolicy::Reject {
        return Err(crate::error::unsupported(
            "link data",
            "opaque link metadata cannot be relocated; select LinkPolicy::Omit to discard it explicitly",
        ));
    }
    Ok(())
}

pub const MAX_POOL_SIZE: usize = 1 << 16;

fn validate_index_limits(plan: &plan::WritePlan<'_>) -> Result<()> {
    let checks: &[(&str, usize)] = &[
        ("type_ids", plan.type_count()),
        ("proto_ids", plan.proto_count()),
        ("field_ids", plan.field_count()),
        ("method_ids", plan.method_count()),
        ("call_site_ids", plan.call_site_count()),
        ("method_handle_ids", plan.method_handle_count()),
    ];
    for &(name, count) in checks {
        if count > MAX_POOL_SIZE {
            return Err(crate::error::invalid(
                "dex",
                format!(
                    "{name} count {count} exceeds maximum {MAX_POOL_SIZE} — \
                     split into multiple DEX files"
                ),
            ));
        }
    }
    Ok(())
}

/// Serializes multiple [`DexFile`]s into a single v41 container buffer.
///
/// Each logical DEX file is written sequentially. All offsets are relative
/// to the start of the physical container.
pub fn write_container(dex_files: &[DexFile]) -> Result<Vec<u8>> {
    write_container_into(dex_files.iter().map(|dex| (dex, None)), Vec::new())
}

/// Streams logical DEX members into one v041 container backed by an anonymous
/// temporary file. Members are emitted in iterator order; an optional part
/// selects a pool-sized subset. Sources remain unchanged and file-backed.
/// Empty input produces an empty file. Invalid members fail the whole write.
pub fn write_container_spooled<'a>(
    members: impl IntoIterator<Item = (&'a DexFile, Option<&'a DexPart>)>,
) -> Result<Spooled> {
    write_container_into(members, SpoolSink::new()?)?.finish()
}

fn write_container_into<'a, S: DexSink>(
    members: impl IntoIterator<Item = (&'a DexFile, Option<&'a DexPart>)>,
    sink: S,
) -> Result<S> {
    let mut w = DexWriter::new(sink);
    let mut spans = Vec::new();
    for (dex, part) in members {
        let plan = plan::WritePlan::new(dex, part, dex.write_options())?;
        validate_index_limits(&plan)?;
        validate_link_policy(&plan)?;
        let header = w.pos();
        w.write_dex(&plan, DexVersion::V041)?;
        spans.push(MemberSpan {
            header,
            end: w.pos(),
        });
    }
    let container_size = w.pos();
    for span in spans {
        w.patch_u32(span.header as usize + 0x70, container_size);
        finalize::sign_member(&mut w, span)?;
    }

    Ok(w.sink)
}

#[derive(Clone, Copy)]
pub(crate) struct MemberSpan {
    pub(crate) header: u32,
    pub(crate) end: u32,
}

pub(crate) struct DexWriter<S: DexSink> {
    pub(crate) sink: S,
    pub(crate) string_data_offsets: Vec<u32>,
    pub(crate) class_data_offsets: Vec<u32>,
    pub(crate) map_entries: Vec<MapItem>,
    pub(crate) header_base: u32,
    pub(crate) version: DexVersion,
    pub(crate) options: WriteOptions,
    pub(crate) original: Option<crate::file::DexBytes>,
}

impl<S: DexSink> DexWriter<S> {
    pub(crate) fn new(sink: S) -> Self {
        Self {
            sink,
            string_data_offsets: Vec::new(),
            class_data_offsets: Vec::new(),
            map_entries: Vec::new(),
            header_base: 0,
            version: DexVersion::V035,
            options: WriteOptions::default(),
            original: None,
        }
    }

    pub(crate) fn pos(&self) -> u32 {
        self.sink.pos()
    }

    pub(crate) fn write(&mut self, bytes: &[u8]) {
        self.sink.write(bytes);
    }

    pub(crate) fn write_zeros(&mut self, count: usize) {
        self.sink.write(&vec![0u8; count]);
    }

    pub(crate) fn align(&mut self, alignment: usize) {
        let padding = (alignment - (self.pos() as usize % alignment)) % alignment;
        self.write_zeros(padding);
    }

    pub(crate) fn write_u16(&mut self, v: u16) {
        self.sink.write(&v.to_le_bytes());
    }
    pub(crate) fn write_u32(&mut self, v: u32) {
        self.sink.write(&v.to_le_bytes());
    }

    pub(crate) fn write_uleb128(&mut self, v: u32) {
        let (bytes, len) = crate::encoding::leb128::encode_uleb128(v);
        self.sink.write(&bytes[..len]);
    }

    pub(crate) fn patch(&mut self, offset: usize, bytes: &[u8]) {
        self.sink.patch(offset, bytes);
    }

    pub(crate) fn patch_u32(&mut self, offset: usize, v: u32) {
        self.sink.patch(offset, &v.to_le_bytes());
    }
}

/// Encodes a sequence of DEX instructions as code units. Operands that exceed
/// their encoding fail. Pool indices must already belong to the owning DEX.
pub use instruction_writer::encode_instructions;
