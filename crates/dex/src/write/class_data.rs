// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::code::{ClassLayout, MethodLayout};
use super::sink::DexSink;
use super::DexWriter;
use crate::types::class::EncodedField;

/// Writes all class-data items and records their file offsets.
///
/// The layouts come from the code pass, so the code offsets are consumed in the
/// same class-then-direct-then-virtual order they were emitted.
pub(crate) fn write_class_data_items<S: DexSink>(
    w: &mut DexWriter<S>,
    layouts: &[Option<ClassLayout>],
) {
    let class_data_start = w.pos();
    let mut class_data_item_count = 0u32;
    w.class_data_offsets.clear();
    let mut code_off_idx = 0usize;
    for layout in layouts {
        if let Some(layout) = layout {
            let off = w.pos();
            w.class_data_offsets.push(off);
            write_class_data(w, layout, &mut code_off_idx);
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
fn write_class_data<S: DexSink>(
    w: &mut DexWriter<S>,
    layout: &ClassLayout,
    code_off_idx: &mut usize,
) {
    w.write_uleb128(layout.static_fields.len() as u32);
    w.write_uleb128(layout.instance_fields.len() as u32);
    w.write_uleb128(layout.direct_methods.len() as u32);
    w.write_uleb128(layout.virtual_methods.len() as u32);

    write_fields(w, &layout.static_fields);
    write_fields(w, &layout.instance_fields);
    write_methods(w, &layout.direct_methods, code_off_idx);
    write_methods(w, &layout.virtual_methods, code_off_idx);
}

fn write_fields<S: DexSink>(w: &mut DexWriter<S>, fields: &[EncodedField]) {
    let mut prev_idx = 0u32;
    for f in fields {
        w.write_uleb128(f.field.0 - prev_idx);
        prev_idx = f.field.0;
        w.write_uleb128(f.access_flags.bits());
    }
}

fn write_methods<S: DexSink>(
    w: &mut DexWriter<S>,
    methods: &[MethodLayout],
    code_off_idx: &mut usize,
) {
    let mut prev_idx = 0u32;
    for m in methods {
        w.write_uleb128(m.method.0 - prev_idx);
        prev_idx = m.method.0;
        w.write_uleb128(m.access_flags.bits());
        let code_off = if m.has_code {
            let off = w.code_item_offsets[*code_off_idx];
            *code_off_idx += 1;
            off
        } else {
            0
        };
        w.write_uleb128(code_off);
    }
}
