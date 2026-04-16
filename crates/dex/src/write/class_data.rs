// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::DexWriter;
use crate::encoding::leb128::write_uleb128;

/// Writes all class-data items and records their file offsets.
pub(crate) fn write_class_data_items(w: &mut DexWriter, dex: &crate::file::DexFile) {
    let class_data_start = w.pos();
    let mut class_data_item_count = 0u32;
    w.class_data_offsets.clear();
    let mut code_off_idx = 0usize;
    for class in &dex.classes {
        if let Some(ref data) = class.class_data {
            let off = w.pos();
            w.class_data_offsets.push(off);
            write_class_data(w, data, &mut code_off_idx);
            class_data_item_count += 1;
        } else {
            w.class_data_offsets.push(0);
        }
    }
    if class_data_item_count > 0 {
        w.map_entries.push(crate::types::map::MapItem {
            type_code: crate::types::map::TYPE_CLASS_DATA_ITEM,
            size: class_data_item_count,
            offset: class_data_start,
        });
    }
}

/// Writes one `class_data_item` using sorted delta-encoded member indexes.
fn write_class_data(
    w: &mut DexWriter,
    data: &crate::types::class::ClassData,
    code_off_idx: &mut usize,
) {
    write_uleb128(&mut w.buf, data.static_fields.len() as u32);
    write_uleb128(&mut w.buf, data.instance_fields.len() as u32);
    write_uleb128(&mut w.buf, data.direct_methods.len() as u32);
    write_uleb128(&mut w.buf, data.virtual_methods.len() as u32);

    let mut prev_idx = 0u32;
    for f in &data.static_fields {
        write_uleb128(&mut w.buf, f.field.0 - prev_idx);
        prev_idx = f.field.0;
        write_uleb128(&mut w.buf, f.access_flags.bits());
    }

    prev_idx = 0;
    for f in &data.instance_fields {
        write_uleb128(&mut w.buf, f.field.0 - prev_idx);
        prev_idx = f.field.0;
        write_uleb128(&mut w.buf, f.access_flags.bits());
    }

    prev_idx = 0;
    for m in &data.direct_methods {
        write_uleb128(&mut w.buf, m.method.0 - prev_idx);
        prev_idx = m.method.0;
        write_uleb128(&mut w.buf, m.access_flags.bits());
        let code_off = if m.code.is_some() {
            let off = w.code_item_offsets[*code_off_idx];
            *code_off_idx += 1;
            off
        } else {
            0
        };
        write_uleb128(&mut w.buf, code_off);
    }

    prev_idx = 0;
    for m in &data.virtual_methods {
        write_uleb128(&mut w.buf, m.method.0 - prev_idx);
        prev_idx = m.method.0;
        write_uleb128(&mut w.buf, m.access_flags.bits());
        let code_off = if m.code.is_some() {
            let off = w.code_item_offsets[*code_off_idx];
            *code_off_idx += 1;
            off
        } else {
            0
        };
        write_uleb128(&mut w.buf, code_off);
    }
}
