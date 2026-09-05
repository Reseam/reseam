// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::encoded_value::write_encoded_array;
use super::intern::StreamInterner;
use super::plan::WritePlan;
use super::sink::DexSink;
use super::DexWriter;
use crate::error::Result;
use crate::file::DexFile;
use crate::types::encoded_value::EncodedValue;
use crate::types::map::{MapItem, TYPE_ENCODED_ARRAY_ITEM, TYPE_TYPE_LIST};
use crate::types::method_handle::CallSiteItem;

/// Writes unique type lists and returns offsets for proto parameters and interfaces.
pub(crate) fn write_type_lists<S: DexSink>(
    w: &mut DexWriter<S>,
    plan: &WritePlan<'_>,
) -> Result<(Vec<u32>, Vec<u32>)> {
    w.align(4);
    let type_lists_off = w.pos();
    let mut lists = StreamInterner::default();
    let mut encoded = Vec::new();

    let mut proto_param_offsets: Vec<u32> = Vec::with_capacity(plan.dex.prototypes.len());
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

/// Writes one `type_list` (padded to keep the section 4-aligned) unless an
/// identical one was written; empty lists have no item and offset 0.
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

/// Writes the hidden-API blob and patches its per-class offset table.
pub(crate) fn write_hidden_api<S: DexSink>(
    w: &mut DexWriter<S>,
    hidden_api: &crate::types::hidden_api::HiddenApiData,
    dex: &DexFile,
) {
    let offset_table_start = w.pos() as usize;
    let class_count = dex.classes.len();
    for _ in 0..class_count {
        w.write_u32(0);
    }

    for (i, flags) in hidden_api.class_flags.iter().enumerate() {
        if let Some(cf) = flags {
            let rel_off = w.pos() as usize - offset_table_start;
            w.patch_u32(offset_table_start + i * 4, rel_off as u32);

            for f in &cf.static_field_flags {
                w.write_uleb128(*f as u32);
            }
            for f in &cf.instance_field_flags {
                w.write_uleb128(*f as u32);
            }
            for f in &cf.direct_method_flags {
                w.write_uleb128(*f as u32);
            }
            for f in &cf.virtual_method_flags {
                w.write_uleb128(*f as u32);
            }
        }
    }
}

/// Writes static-value arrays and call-site payload arrays.
pub(crate) fn write_encoded_arrays<S: DexSink>(
    w: &mut DexWriter<S>,
    plan: &WritePlan<'_>,
    call_sites: &[CallSiteItem],
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
    if encoded_array_count > 0 {
        w.map_entries.push(MapItem {
            type_code: TYPE_ENCODED_ARRAY_ITEM,
            size: encoded_array_count,
            offset: encoded_arrays_start,
        });
    }

    let mut call_site_data_offsets: Vec<u32> = Vec::with_capacity(call_sites.len());
    for cs in call_sites {
        let off = w.pos();
        call_site_data_offsets.push(off);
        let mut values = vec![
            EncodedValue::MethodHandle(cs.bootstrap_method),
            EncodedValue::String(cs.method_name),
            EncodedValue::MethodType(cs.method_type),
        ];
        values.extend(cs.extra_arguments.iter().cloned());
        array.clear();
        write_encoded_array(&mut array, &values);
        w.write(&array);
    }

    Ok((static_values_offsets, call_site_data_offsets))
}
