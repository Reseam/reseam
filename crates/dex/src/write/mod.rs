// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! DEX serialization entrypoints and helpers.

pub use self::layout::WriteLayout;
pub use self::part::{DexPart, split_to_fit};
pub use self::sink::{DexSink, SpoolSink, Spooled};
use crate::error::Result;
use crate::file::DexFile;
use crate::model::Pool;
use crate::model::encoded_value::EncodedValue;
use crate::model::header::DexVersion;
use crate::model::map::MapItem;
use crate::options::{LinkPolicy, MAX_POOL_SIZE, WriteOptions};

pub(crate) mod annotations;
pub(crate) mod class_data;
pub(crate) mod code;
pub(crate) mod debug;
pub(crate) mod encoded_arrays;
pub(crate) mod encoded_value;
pub(crate) mod finalize;
mod hidden_api;
mod ids;
pub(crate) mod intern;
pub(crate) mod layout;
pub(crate) mod part;
pub(crate) mod plan;
pub(crate) mod raw_code;
pub(crate) mod remap;
pub(crate) mod sections;
pub(crate) mod sink;
mod type_lists;

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

/// Class `class_idx` serialized as a DEX of its own. Every id pool of a DEX is
/// sorted, so two definitions of a class written by the same DEX version yield
/// the same bytes exactly when they are equivalent, whichever files hold them.
pub fn write_class(dex: &DexFile, class_idx: usize) -> Result<Vec<u8>> {
    Ok(write_into(dex, Some(&part::of_class(dex, class_idx)?), Vec::new())?.0)
}

/// Serializes into an anonymous temp file instead of memory, optionally only
/// one [`DexPart`] of the file, with where its pools and classes landed.
pub fn write_spooled(dex: &DexFile, part: Option<&DexPart>) -> Result<(Spooled, WriteLayout)> {
    let (sink, layout) = write_into(
        dex,
        part,
        SpoolSink::new().map_err(crate::error::DexError::Io)?,
    )?;
    Ok((sink.finish()?, layout))
}

fn write_into<S: DexSink>(
    dex: &DexFile,
    part: Option<&DexPart>,
    sink: S,
) -> Result<(S, WriteLayout)> {
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
    Ok((w.sink, WriteLayout::of(plan)))
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

fn validate_index_limits(plan: &plan::WritePlan<'_>) -> Result<()> {
    for pool in Pool::ALL.into_iter().filter(|&pool| pool != Pool::String) {
        let count = plan.count(pool);
        if count > MAX_POOL_SIZE {
            return Err(crate::error::invalid(
                "dex",
                format!(
                    "{pool:?} id count {count} exceeds maximum {MAX_POOL_SIZE}; \
                     split into multiple DEX files"
                ),
            ));
        }
    }
    Ok(())
}

/// Streams logical DEX members into one v041 container backed by an anonymous
/// temporary file. Members are emitted in iterator order; an optional part
/// selects a pool-sized subset. Sources remain unchanged and file-backed.
/// Empty input produces an empty file. Invalid members fail the whole write.
/// Layouts follow member order.
pub fn write_container_spooled<'a>(
    members: impl IntoIterator<Item = (&'a DexFile, Option<&'a DexPart>)>,
) -> Result<(Spooled, Vec<WriteLayout>)> {
    let (sink, layouts) = write_container_into(members, SpoolSink::new()?)?;
    Ok((sink.finish()?, layouts))
}

fn write_container_into<'a, S: DexSink>(
    members: impl IntoIterator<Item = (&'a DexFile, Option<&'a DexPart>)>,
    sink: S,
) -> Result<(S, Vec<WriteLayout>)> {
    let mut w = DexWriter::new(sink);
    let mut spans = Vec::new();
    let mut layouts = Vec::new();
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
        layouts.push(WriteLayout::of(plan));
    }
    let container_size = w.pos();
    for span in spans {
        w.patch_u32(span.header as usize + 0x70, container_size);
        finalize::sign_member(&mut w, span)?;
    }

    Ok((w.sink, layouts))
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
    pub(crate) original: Option<crate::DexBytes>,
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

    /// Lists a section of `size` items at `offset` in the map; an empty
    /// section is not listed.
    pub(crate) fn map_section(&mut self, type_code: u16, offset: u32, size: u32) {
        if size > 0 {
            self.map_entries.push(MapItem {
                type_code,
                size,
                offset,
            });
        }
    }

    /// Writes a section of `size` items and lists it in the map, returning where it starts.
    pub(crate) fn section(
        &mut self,
        type_code: u16,
        size: usize,
        write: impl FnOnce(&mut Self),
    ) -> u32 {
        let offset = self.pos();
        write(self);
        self.map_section(type_code, offset, size as u32);
        offset
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
