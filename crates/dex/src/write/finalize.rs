// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::plan::WritePlan;
use crate::error::Result;
use crate::types::class::NO_INDEX;
use zlib_rs::adler32::{adler32, adler32_combine};

use super::sink::DexSink;
use super::{DexWriter, MemberSpan};

#[derive(Clone, Copy)]
pub(super) struct IdOffsets {
    pub strings: u32,
    pub types: u32,
    pub prototypes: u32,
    pub fields: u32,
    pub methods: u32,
    pub classes: u32,
    pub call_sites: Option<u32>,
}

#[derive(Clone, Copy)]
pub(super) struct DataOffsets<'a> {
    pub start: u32,
    pub map: u32,
    pub annotations: &'a [u32],
    pub prototype_parameters: &'a [u32],
    pub interfaces: &'a [u32],
    pub static_values: &'a [u32],
    pub call_sites: &'a [u32],
}

pub(super) fn finalize<S: DexSink>(
    w: &mut DexWriter<S>,
    plan: &WritePlan<'_>,
    ids: IdOffsets,
    data: DataOffsets<'_>,
) {
    for i in 0..w.string_data_offsets.len() {
        w.patch_u32(ids.strings as usize + i * 4, w.string_data_offsets[i]);
    }
    for (i, proto) in plan.prototypes().enumerate() {
        let base = ids.prototypes as usize + i * 12;
        w.patch_u32(base, proto.shorty.0);
        w.patch_u32(base + 4, proto.return_type.0);
        w.patch_u32(base + 8, data.prototype_parameters[i]);
    }
    for i in 0..plan.classes.len() {
        let class = plan.class_header(i);
        let base = ids.classes as usize + i * 32;
        w.patch_u32(base, class.class_type.0);
        w.patch_u32(base + 4, class.access_flags.bits());
        w.patch_u32(base + 8, class.superclass.map_or(NO_INDEX, |t| t.0));
        w.patch_u32(base + 12, data.interfaces[i]);
        w.patch_u32(base + 16, class.source_file.map_or(NO_INDEX, |s| s.0));
        w.patch_u32(base + 20, data.annotations[i]);
        w.patch_u32(base + 24, w.class_data_offsets[i]);
        w.patch_u32(base + 28, data.static_values[i]);
    }
    if let Some(off) = ids.call_sites {
        for (index, &data_off) in data.call_sites.iter().enumerate() {
            w.patch_u32(off as usize + index * 4, data_off);
        }
    }
    write_header(w, plan, &ids, &data);
}

fn write_header<S: DexSink>(
    w: &mut DexWriter<S>,
    plan: &WritePlan<'_>,
    ids: &IdOffsets,
    data: &DataOffsets<'_>,
) {
    let base = w.header_base as usize;
    let version = w.version;
    w.patch(base, version.magic_bytes());
    w.patch_u32(base + 0x20, w.pos() - w.header_base);
    w.patch_u32(base + 0x24, version.header_size());
    w.patch_u32(base + 0x28, 0x1234_5678);
    w.patch_u32(base + 0x2c, 0);
    w.patch_u32(base + 0x30, 0);
    w.patch_u32(base + 0x34, data.map);
    for (index, (count, offset)) in [
        (plan.string_count(), ids.strings),
        (plan.type_count(), ids.types),
        (plan.proto_count(), ids.prototypes),
        (plan.field_count(), ids.fields),
        (plan.method_count(), ids.methods),
        (plan.classes.len(), ids.classes),
    ]
    .into_iter()
    .enumerate()
    {
        w.patch_u32(base + 0x38 + index * 8, count as u32);
        w.patch_u32(base + 0x3c + index * 8, if count == 0 { 0 } else { offset });
    }
    if version.is_container_format() {
        w.patch_u32(base + 0x68, 0);
        w.patch_u32(base + 0x6c, 0);
        w.patch_u32(base + 0x70, w.pos());
        w.patch_u32(base + 0x74, w.header_base);
    } else {
        let size = (w.pos() - data.start + 3) & !3;
        w.patch_u32(base + 0x68, size);
        w.patch_u32(base + 0x6c, data.start);
    }
}

pub(crate) fn sign_member<S: DexSink>(w: &mut DexWriter<S>, span: MemberSpan) -> Result<()> {
    let header_base = span.header as usize;
    let logical_end = span.end as usize;

    let mut signature = ring::digest::Context::new(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY);
    let mut body_checksum = 1;
    w.sink.digest(header_base + 32, logical_end, &mut |chunk| {
        signature.update(chunk);
        body_checksum = adler32(body_checksum, chunk);
    })?;
    let signature = signature.finish();
    w.patch(header_base + 0x0c, signature.as_ref());
    // The checksum covers the signature too, which is only known now.
    let checksum = adler32_combine(
        adler32(1, signature.as_ref()),
        body_checksum,
        (logical_end - header_base - 32) as u64,
    );
    w.patch_u32(header_base + 0x08, checksum);

    Ok(())
}
