// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::encoded_value::write_encoded_array;
use super::DexWriter;
use crate::encoding::leb128::write_uleb128;
use crate::file::DexFile;
use crate::types::encoded_value::EncodedValue;
use crate::types::map::{MapItem, TYPE_ENCODED_ARRAY_ITEM, TYPE_TYPE_LIST};

/// Writes unique type lists and returns offsets for proto parameters and interfaces.
pub(crate) fn write_type_lists(w: &mut DexWriter, dex: &DexFile) -> (Vec<u32>, Vec<u32>) {
    let mut type_list_count = 0u32;
    let type_lists_off = w.pos();
    let mut proto_param_offsets: Vec<u32> = Vec::new();
    for proto in &dex.prototypes {
        if proto.parameters.is_empty() {
            proto_param_offsets.push(0);
        } else if let Some(&cached) = w.type_list_cache.get(&proto.parameters) {
            proto_param_offsets.push(cached);
        } else {
            w.align(4);
            let off = w.pos();
            write_type_list(w, &proto.parameters);
            w.type_list_cache.insert(proto.parameters.clone(), off);
            proto_param_offsets.push(off);
            type_list_count += 1;
        }
    }

    let mut class_interface_offsets: Vec<u32> = Vec::new();
    for class in &dex.classes {
        if class.interfaces.is_empty() {
            class_interface_offsets.push(0);
        } else if let Some(&cached) = w.type_list_cache.get(&class.interfaces) {
            class_interface_offsets.push(cached);
        } else {
            w.align(4);
            let off = w.pos();
            write_type_list(w, &class.interfaces);
            w.type_list_cache.insert(class.interfaces.clone(), off);
            class_interface_offsets.push(off);
            type_list_count += 1;
        }
    }

    if type_list_count > 0 {
        w.map_entries.push(MapItem {
            type_code: TYPE_TYPE_LIST,
            size: type_list_count,
            offset: type_lists_off,
        });
    }
    (proto_param_offsets, class_interface_offsets)
}

/// Writes the hidden-API blob and patches its per-class offset table.
pub(crate) fn write_hidden_api(
    w: &mut DexWriter,
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
                write_uleb128(&mut w.buf, *f as u32);
            }
            for f in &cf.instance_field_flags {
                write_uleb128(&mut w.buf, *f as u32);
            }
            for f in &cf.direct_method_flags {
                write_uleb128(&mut w.buf, *f as u32);
            }
            for f in &cf.virtual_method_flags {
                write_uleb128(&mut w.buf, *f as u32);
            }
        }
    }
}

/// Writes static-value arrays and call-site payload arrays.
pub(crate) fn write_encoded_arrays(w: &mut DexWriter, dex: &DexFile) -> (Vec<u32>, Vec<u32>) {
    let mut encoded_array_count = 0u32;
    let encoded_arrays_start = w.pos();
    let mut static_values_offsets: Vec<u32> = Vec::new();
    for class in &dex.classes {
        if class.static_values.is_empty() {
            static_values_offsets.push(0);
        } else {
            let mut last_non_default = class.static_values.len();
            while last_non_default > 0
                && super::is_default_value(&class.static_values[last_non_default - 1])
            {
                last_non_default -= 1;
            }
            if last_non_default == 0 {
                static_values_offsets.push(0);
            } else {
                let off = w.pos();
                static_values_offsets.push(off);
                write_encoded_array(&mut w.buf, &class.static_values[..last_non_default]);
                encoded_array_count += 1;
            }
        }
    }
    if encoded_array_count > 0 {
        w.map_entries.push(MapItem {
            type_code: TYPE_ENCODED_ARRAY_ITEM,
            size: encoded_array_count,
            offset: encoded_arrays_start,
        });
    }

    let mut call_site_data_offsets: Vec<u32> = Vec::new();
    for cs in &dex.call_sites {
        let off = w.pos();
        call_site_data_offsets.push(off);
        let mut values = vec![
            EncodedValue::MethodHandle(cs.bootstrap_method),
            EncodedValue::String(cs.method_name),
            EncodedValue::MethodType(cs.method_type),
        ];
        values.extend(cs.extra_arguments.iter().cloned());
        write_encoded_array(&mut w.buf, &values);
    }

    (static_values_offsets, call_site_data_offsets)
}

/// Writes one `type_list`, including the required trailing padding word.
fn write_type_list(w: &mut DexWriter, types: &[crate::types::TypeIdx]) {
    w.write_u32(types.len() as u32);
    for t in types {
        w.write_u16(t.0 as u16);
    }
    if !types.len().is_multiple_of(2) {
        w.write_u16(0);
    }
}
