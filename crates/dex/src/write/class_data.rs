// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::DexWriter;
use super::sink::DexSink;
use crate::model::class::EncodedField;
use crate::model::map::TYPE_CLASS_DATA_ITEM;
use crate::read::class::{ClassSkeleton, MethodHeader};

pub(crate) fn write_class_data_items<S: DexSink>(
    w: &mut DexWriter<S>,
    layouts: &[Option<ClassSkeleton>],
) {
    let class_data_start = w.pos();
    let mut class_data_item_count = 0u32;
    w.class_data_offsets.clear();
    for layout in layouts {
        if let Some(layout) = layout {
            let off = w.pos();
            w.class_data_offsets.push(off);
            write_class_data(w, layout);
            class_data_item_count += 1;
        } else {
            w.class_data_offsets.push(0);
        }
    }
    w.map_section(
        TYPE_CLASS_DATA_ITEM,
        class_data_start,
        class_data_item_count,
    );
}

fn write_class_data<S: DexSink>(w: &mut DexWriter<S>, layout: &ClassSkeleton) {
    w.write_uleb128(layout.static_fields.len() as u32);
    w.write_uleb128(layout.instance_fields.len() as u32);
    w.write_uleb128(layout.direct_methods.len() as u32);
    w.write_uleb128(layout.virtual_methods.len() as u32);

    write_fields(w, &layout.static_fields);
    write_fields(w, &layout.instance_fields);
    write_methods(w, &layout.direct_methods);
    write_methods(w, &layout.virtual_methods);
}

fn write_fields<S: DexSink>(w: &mut DexWriter<S>, fields: &[EncodedField]) {
    let mut prev_idx = 0u32;
    for f in fields {
        w.write_uleb128(f.field.0 - prev_idx);
        prev_idx = f.field.0;
        w.write_uleb128(f.access_flags.bits());
    }
}

fn write_methods<S: DexSink>(w: &mut DexWriter<S>, methods: &[MethodHeader]) {
    let mut prev_idx = 0u32;
    for m in methods {
        w.write_uleb128(m.method.0 - prev_idx);
        prev_idx = m.method.0;
        w.write_uleb128(m.access_flags.bits());
        w.write_uleb128(m.code_off);
    }
}
