// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::DexWriter;
use super::intern::StreamInterner;
use super::plan::WritePlan;
use super::sink::DexSink;
use crate::error::Result;
use crate::model::Pool;
use crate::model::map::TYPE_TYPE_LIST;

pub(crate) fn write_type_lists<S: DexSink>(
    w: &mut DexWriter<S>,
    plan: &WritePlan<'_>,
) -> Result<(Vec<u32>, Vec<u32>)> {
    w.align(4);
    let type_lists_off = w.pos();
    let mut lists = StreamInterner::default();
    let mut encoded = Vec::new();

    let mut proto_param_offsets: Vec<u32> = Vec::with_capacity(plan.count(Pool::Proto));
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

    w.map_section(TYPE_TYPE_LIST, type_lists_off, lists.len() as u32);
    Ok((proto_param_offsets, class_interface_offsets))
}

fn intern_type_list<S: DexSink>(
    w: &mut DexWriter<S>,
    lists: &mut StreamInterner,
    encoded: &mut Vec<u8>,
    types: &[crate::model::TypeIdx],
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
