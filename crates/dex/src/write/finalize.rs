// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::plan::WritePlan;
use crate::error::Result;
use crate::types::class::NO_INDEX;
use sha1::{Digest, Sha1};

use super::annotations::ClassAnnotations;
use super::sink::DexSink;
use super::DexWriter;

#[allow(clippy::too_many_arguments)]
pub(crate) fn finalize<S: DexSink>(
    w: &mut DexWriter<S>,
    plan: &WritePlan<'_>,
    data_off: u32,
    data_size: u32,
    string_ids_off: u32,
    type_ids_off: u32,
    proto_ids_off: u32,
    field_ids_off: u32,
    method_ids_off: u32,
    class_defs_off: u32,
    map_off: u32,
    class_ann_datas: &ClassAnnotations,
    proto_param_offsets: &[u32],
    class_interface_offsets: &[u32],
    static_values_offsets: &[u32],
    call_site_data_offsets: &[u32],
    call_sites_off: Option<u32>,
) -> Result<()> {
    for i in 0..w.string_data_offsets.len() {
        w.patch_u32(string_ids_off as usize + i * 4, w.string_data_offsets[i]);
    }

    let dex = plan.dex;
    for (i, proto) in plan.prototypes().enumerate() {
        let base = proto_ids_off as usize + i * 12;
        w.patch_u32(base, proto.shorty.0);
        w.patch_u32(base + 4, proto.return_type.0);
        w.patch_u32(base + 8, proto_param_offsets[i]);
    }

    for i in 0..plan.classes.len() {
        let class = plan.class_header(i);
        let base = class_defs_off as usize + i * 32;
        w.patch_u32(base, class.class_type.0);
        w.patch_u32(base + 4, class.access_flags.bits());
        w.patch_u32(base + 8, class.superclass.map(|t| t.0).unwrap_or(NO_INDEX));
        w.patch_u32(base + 12, class_interface_offsets[i]);
        w.patch_u32(
            base + 16,
            class.source_file.map(|s| s.0).unwrap_or(NO_INDEX),
        );
        let ann_off = class_ann_datas[i]
            .as_ref()
            .map(|(off, _)| *off)
            .unwrap_or(0);
        w.patch_u32(base + 20, ann_off);
        w.patch_u32(base + 24, w.class_data_offsets[i]);
        w.patch_u32(base + 28, static_values_offsets[i]);
    }

    if let Some(cs_off) = call_sites_off {
        for (i, off) in call_site_data_offsets.iter().enumerate() {
            w.patch_u32(cs_off as usize + i * 4, *off);
        }
    }

    let version = dex.required_version();
    let is_container = version.is_container_format();
    let header_size = version.header_size();

    const OFF_CHECKSUM: usize = 0x08;
    const OFF_SIGNATURE: usize = 0x0C;
    const OFF_FILE_SIZE: usize = 0x20;
    const OFF_HEADER_SIZE: usize = 0x24;
    const OFF_ENDIAN_TAG: usize = 0x28;
    const OFF_LINK_SIZE: usize = 0x2C;
    const OFF_LINK_OFF: usize = 0x30;
    const OFF_MAP_OFF: usize = 0x34;
    const OFF_STRING_IDS_SIZE: usize = 0x38;
    const OFF_STRING_IDS_OFF: usize = 0x3C;
    const OFF_TYPE_IDS_SIZE: usize = 0x40;
    const OFF_TYPE_IDS_OFF: usize = 0x44;
    const OFF_PROTO_IDS_SIZE: usize = 0x48;
    const OFF_PROTO_IDS_OFF: usize = 0x4C;
    const OFF_FIELD_IDS_SIZE: usize = 0x50;
    const OFF_FIELD_IDS_OFF: usize = 0x54;
    const OFF_METHOD_IDS_SIZE: usize = 0x58;
    const OFF_METHOD_IDS_OFF: usize = 0x5C;
    const OFF_CLASS_DEFS_SIZE: usize = 0x60;
    const OFF_CLASS_DEFS_OFF: usize = 0x64;
    const OFF_DATA_SIZE: usize = 0x68;
    const OFF_DATA_OFF: usize = 0x6C;
    const ENDIAN_TAG: u32 = 0x12345678;

    let header_base = w.header_base as usize;

    w.patch(header_base, version.magic_bytes());
    w.patch_u32(header_base + OFF_HEADER_SIZE, header_size);
    w.patch_u32(header_base + OFF_ENDIAN_TAG, ENDIAN_TAG);
    w.patch_u32(header_base + OFF_LINK_SIZE, 0);
    w.patch_u32(header_base + OFF_LINK_OFF, 0);
    w.patch_u32(header_base + OFF_MAP_OFF, map_off);
    w.patch_u32(header_base + OFF_STRING_IDS_SIZE, dex.strings.len() as u32);
    w.patch_u32(
        header_base + OFF_STRING_IDS_OFF,
        if !dex.strings.is_empty() {
            string_ids_off
        } else {
            0
        },
    );
    w.patch_u32(header_base + OFF_TYPE_IDS_SIZE, dex.types.len() as u32);
    w.patch_u32(
        header_base + OFF_TYPE_IDS_OFF,
        if !dex.types.is_empty() {
            type_ids_off
        } else {
            0
        },
    );
    w.patch_u32(
        header_base + OFF_PROTO_IDS_SIZE,
        dex.prototypes.len() as u32,
    );
    w.patch_u32(
        header_base + OFF_PROTO_IDS_OFF,
        if !dex.prototypes.is_empty() {
            proto_ids_off
        } else {
            0
        },
    );
    w.patch_u32(header_base + OFF_FIELD_IDS_SIZE, dex.fields.len() as u32);
    w.patch_u32(
        header_base + OFF_FIELD_IDS_OFF,
        if !dex.fields.is_empty() {
            field_ids_off
        } else {
            0
        },
    );
    w.patch_u32(header_base + OFF_METHOD_IDS_SIZE, dex.methods.len() as u32);
    w.patch_u32(
        header_base + OFF_METHOD_IDS_OFF,
        if !dex.methods.is_empty() {
            method_ids_off
        } else {
            0
        },
    );
    w.patch_u32(header_base + OFF_CLASS_DEFS_SIZE, plan.classes.len() as u32);
    w.patch_u32(
        header_base + OFF_CLASS_DEFS_OFF,
        if !plan.classes.is_empty() {
            class_defs_off
        } else {
            0
        },
    );

    if is_container {
        w.patch_u32(header_base + OFF_DATA_SIZE, 0);
        w.patch_u32(header_base + OFF_DATA_OFF, 0);
    } else {
        let aligned_data_size = (data_size + 3) & !3;
        w.patch_u32(header_base + OFF_DATA_SIZE, aligned_data_size);
        w.patch_u32(header_base + OFF_DATA_OFF, data_off);
    }

    let file_size = w.pos() - w.header_base;
    w.patch_u32(header_base + OFF_FILE_SIZE, file_size);

    if is_container {
        w.patch_u32(header_base + 0x70, w.container_size);
        w.patch_u32(header_base + 0x74, w.header_base);
    }

    let logical_end = w.pos() as usize;

    let mut hasher = Sha1::new();
    w.sink.digest(header_base + 32, logical_end, &mut |chunk| {
        hasher.update(chunk)
    })?;
    let sig: [u8; 20] = hasher.finalize().into();
    w.patch(header_base + OFF_SIGNATURE, &sig);

    let mut adler = adler::Adler32::new();
    w.sink
        .digest(header_base + OFF_CHECKSUM + 4, logical_end, &mut |chunk| {
            adler.write_slice(chunk)
        })?;
    w.patch_u32(header_base + OFF_CHECKSUM, adler.checksum());

    Ok(())
}
