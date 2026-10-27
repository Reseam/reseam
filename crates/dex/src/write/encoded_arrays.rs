// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::DexWriter;
use super::encoded_value::write_encoded_array;
use super::plan::WritePlan;
use super::sink::DexSink;
use crate::error::Result;
use crate::model::Pool;
use crate::model::encoded_value::EncodedValue;
use crate::model::map::TYPE_ENCODED_ARRAY_ITEM;

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
    let mut call_site_data_offsets: Vec<u32> = Vec::with_capacity(plan.count(Pool::CallSite));
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

    w.map_section(
        TYPE_ENCODED_ARRAY_ITEM,
        encoded_arrays_start,
        encoded_array_count,
    );

    Ok((static_values_offsets, call_site_data_offsets))
}
