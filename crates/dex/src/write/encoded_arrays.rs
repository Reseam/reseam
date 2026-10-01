// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::DexWriter;
use super::encoded_value::write_encoded_array;
use super::intern::StreamInterner;
use super::plan::WritePlan;
use super::sink::DexSink;
use crate::error::Result;
use crate::types::encoded_value::EncodedValue;
use crate::types::map::{MapItem, TYPE_ENCODED_ARRAY_ITEM, TYPE_TYPE_LIST};

pub(crate) fn write_type_lists<S: DexSink>(
    w: &mut DexWriter<S>,
    plan: &WritePlan<'_>,
) -> Result<(Vec<u32>, Vec<u32>)> {
    w.align(4);
    let type_lists_off = w.pos();
    let mut lists = StreamInterner::default();
    let mut encoded = Vec::new();

    let mut proto_param_offsets: Vec<u32> = Vec::with_capacity(plan.proto_count());
    for proto in plan.prototypes() {
        proto_param_offsets.push(intern_type_list(
            w,
            &mut lists,
            &mut encoded,
            &proto.parameters,
        )?);
    }

    let mut class_interface_offsets: Vec<u32> = Vec::with_capacity(plan.classes.len());
    for k in 0..plan.classes.len() {
        let interfaces = plan.class_interfaces(k);
        class_interface_offsets.push(intern_type_list(w, &mut lists, &mut encoded, &interfaces)?);
    }

    if lists.len() > 0 {
        w.map_entries.push(MapItem {
            type_code: TYPE_TYPE_LIST,
            size: lists.len() as u32,
            offset: type_lists_off,
        });
    }
    Ok((proto_param_offsets, class_interface_offsets))
}

fn intern_type_list<S: DexSink>(
    w: &mut DexWriter<S>,
    lists: &mut StreamInterner,
    encoded: &mut Vec<u8>,
    types: &[crate::types::TypeIdx],
) -> Result<u32> {
    if types.is_empty() {
        return Ok(0);
    }
    encoded.clear();
    encoded.extend_from_slice(&(types.len() as u32).to_le_bytes());
    for t in types {
        encoded.extend_from_slice(&(t.0 as u16).to_le_bytes());
    }
    if !types.len().is_multiple_of(2) {
        encoded.extend_from_slice(&[0, 0]);
    }
    lists.intern(&mut w.sink, encoded)
}

pub(crate) fn write_hidden_api<S: DexSink>(
    w: &mut DexWriter<S>,
    hidden_api: &crate::HiddenApiData,
    plan: &WritePlan<'_>,
) -> Result<()> {
    let table_start = w.pos() as usize;
    w.write_u32(0);
    for _ in &plan.classes {
        w.write_u32(0);
    }
    for (i, &source) in plan.class_order.iter().enumerate() {
        let ty = plan.dex.class_header(source).class_type;
        let Some(flags) = hidden_api.get(ty)? else {
            continue;
        };
        let relative = w.pos() as usize - table_start;
        w.patch_u32(table_start + 4 + i * 4, relative as u32);
        let fields = plan.dex.decode_class_fields(source)?.unwrap_or_default();
        for mut group in [fields.0, fields.1] {
            group.sort_by_key(|field| {
                plan.remap()
                    .map_or(field.field, |remap| remap.remap_field(field.field))
            });
            for field in group {
                w.write_uleb128(
                    flags
                        .field_flags
                        .get(&field.field)
                        .copied()
                        .unwrap_or(crate::HiddenApiFlags::SDK)
                        .bits(),
                );
            }
        }
        let groups = if let Some(class) = plan.dex.resident_class(source) {
            class.class_data.as_ref().map_or_else(
                || [Vec::new(), Vec::new()],
                |data| {
                    [
                        data.direct_methods.iter().map(|m| m.method).collect(),
                        data.virtual_methods.iter().map(|m| m.method).collect(),
                    ]
                },
            )
        } else if let Some(skeleton) = plan.dex.class_skeleton(source)? {
            [
                skeleton.direct_methods.iter().map(|m| m.method).collect(),
                skeleton.virtual_methods.iter().map(|m| m.method).collect(),
            ]
        } else {
            [Vec::new(), Vec::new()]
        };
        for mut group in groups {
            group.sort_by_key(|method| {
                plan.remap()
                    .map_or(*method, |remap| remap.remap_method(*method))
            });
            for method in group {
                w.write_uleb128(
                    flags
                        .method_flags
                        .get(&method)
                        .copied()
                        .unwrap_or(crate::HiddenApiFlags::SDK)
                        .bits(),
                );
            }
        }
    }
    w.patch_u32(table_start, w.pos() - table_start as u32);
    Ok(())
}

pub(crate) fn write_encoded_arrays<S: DexSink>(
    w: &mut DexWriter<S>,
    plan: &WritePlan<'_>,
) -> Result<(Vec<u32>, Vec<u32>)> {
    let mut encoded_array_count = 0u32;
    let encoded_arrays_start = w.pos();
    let mut static_values_offsets: Vec<u32> = Vec::with_capacity(plan.classes.len());
    let mut array = Vec::new();
    for k in 0..plan.classes.len() {
        let static_values = plan.class_static_values(k)?;
        let mut last_non_default = static_values.len();
        while last_non_default > 0 && super::is_default_value(&static_values[last_non_default - 1])
        {
            last_non_default -= 1;
        }
        if last_non_default == 0 {
            static_values_offsets.push(0);
        } else {
            let off = w.pos();
            static_values_offsets.push(off);
            array.clear();
            write_encoded_array(&mut array, &static_values[..last_non_default]);
            w.write(&array);
            encoded_array_count += 1;
        }
    }
    let mut call_site_data_offsets: Vec<u32> = Vec::with_capacity(plan.call_site_count());
    for cs in plan.call_sites() {
        let cs = cs?;
        let off = w.pos();
        call_site_data_offsets.push(off);
        let mut values = vec![
            EncodedValue::MethodHandle(cs.bootstrap_method),
            EncodedValue::String(cs.method_name),
            EncodedValue::MethodType(cs.method_type),
        ];
        values.extend(cs.into_owned().extra_arguments);
        array.clear();
        write_encoded_array(&mut array, &values);
        w.write(&array);
        encoded_array_count += 1;
    }

    if encoded_array_count > 0 {
        w.map_entries.push(MapItem {
            type_code: TYPE_ENCODED_ARRAY_ITEM,
            size: encoded_array_count,
            offset: encoded_arrays_start,
        });
    }

    Ok((static_values_offsets, call_site_data_offsets))
}
