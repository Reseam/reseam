// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::DexWriter;
use super::plan::WritePlan;
use super::sink::DexSink;
use crate::error::Result;

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
        let ty = plan.dex.classes().header(source).class_type;
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
        let groups = if let Some(class) = plan.dex.classes().resident(source) {
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
