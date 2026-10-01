// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::DexWriter;
use super::plan::WritePlan;
use super::sink::DexSink;
use super::{annotations, class_data, code, encoded_arrays, finalize};
use crate::error::Result;
use crate::types::map::{
    MapItem, TYPE_CALL_SITE_ID_ITEM, TYPE_CLASS_DEF_ITEM, TYPE_FIELD_ID_ITEM, TYPE_HEADER_ITEM,
    TYPE_MAP_LIST, TYPE_METHOD_HANDLE_ITEM, TYPE_METHOD_ID_ITEM, TYPE_PROTO_ID_ITEM,
    TYPE_STRING_DATA_ITEM, TYPE_STRING_ID_ITEM, TYPE_TYPE_ID_ITEM,
};

impl<S: DexSink> DexWriter<S> {
    pub(crate) fn write_dex(
        &mut self,
        plan: &WritePlan<'_>,
        version: crate::types::header::DexVersion,
    ) -> Result<()> {
        self.version = version;
        let dex = plan.dex;
        self.options = plan.options;
        self.original.clone_from(&dex.raw);
        let header_off = self.pos();
        self.header_base = header_off;
        let header_size = version.header_size() as usize;
        self.write_zeros(header_size);
        self.map_entries.clear();
        self.map_entries.push(MapItem {
            type_code: TYPE_HEADER_ITEM,
            size: 1,
            offset: header_off,
        });

        let ids = self.write_ids(plan)?;
        let data_off = self.pos();

        let (proto_param_offsets, class_interface_offsets) =
            encoded_arrays::write_type_lists(self, plan)?;

        let class_ann_datas = annotations::write_annotations(self, plan)?;

        let layouts = code::write_code_and_debug(self, plan)?;
        class_data::write_class_data_items(self, &layouts);

        let string_data_start = self.pos();
        self.string_data_offsets.clear();
        for i in 0..plan.string_count() {
            let off = self.pos();
            self.string_data_offsets.push(off);
            self.write(&plan.string_item(i));
        }
        if plan.string_count() > 0 {
            self.map_entries.push(MapItem {
                type_code: TYPE_STRING_DATA_ITEM,
                size: plan.string_count() as u32,
                offset: string_data_start,
            });
        }

        let (static_values_offsets, call_site_data_offsets) =
            encoded_arrays::write_encoded_arrays(self, plan)?;

        if let Some(ref hidden_api) = dex.hidden_api {
            self.align(4);
            let hidden_api_off = self.pos();
            encoded_arrays::write_hidden_api(self, hidden_api, plan)?;
            self.map_entries.push(MapItem {
                type_code: crate::types::map::TYPE_HIDDENAPI_CLASS_DATA_ITEM,
                size: 1,
                offset: hidden_api_off,
            });
        }

        self.align(4);
        let map_off = self.pos();
        self.map_entries.push(MapItem {
            type_code: TYPE_MAP_LIST,
            size: 1,
            offset: map_off,
        });
        self.map_entries.sort_by_key(|e| e.offset);
        let map_entries = std::mem::take(&mut self.map_entries);
        self.write_u32(map_entries.len() as u32);
        for entry in &map_entries {
            self.write_u16(entry.type_code);
            self.write_u16(0);
            self.write_u32(entry.size);
            self.write_u32(entry.offset);
        }

        finalize::finalize(
            self,
            plan,
            ids,
            finalize::DataOffsets {
                start: data_off,
                map: map_off,
                annotations: &class_ann_datas,
                prototype_parameters: &proto_param_offsets,
                interfaces: &class_interface_offsets,
                static_values: &static_values_offsets,
                call_sites: &call_site_data_offsets,
            },
        );
        Ok(())
    }
    fn write_ids(&mut self, plan: &WritePlan<'_>) -> Result<finalize::IdOffsets> {
        let string_ids_off = self.pos();
        let string_count = plan.string_count() as u32;
        for _ in 0..string_count {
            self.write_u32(0);
        }
        if string_count > 0 {
            self.map_entries.push(MapItem {
                type_code: TYPE_STRING_ID_ITEM,
                size: string_count,
                offset: string_ids_off,
            });
        }

        let type_ids_off = self.pos();
        let type_count = plan.type_count() as u32;
        for desc_idx in plan.types() {
            self.write_u32(desc_idx.0);
        }
        if type_count > 0 {
            self.map_entries.push(MapItem {
                type_code: TYPE_TYPE_ID_ITEM,
                size: type_count,
                offset: type_ids_off,
            });
        }

        let proto_ids_off = self.pos();
        let proto_count = plan.proto_count() as u32;
        for _ in 0..proto_count {
            self.write_u32(0);
            self.write_u32(0);
            self.write_u32(0);
        }
        if proto_count > 0 {
            self.map_entries.push(MapItem {
                type_code: TYPE_PROTO_ID_ITEM,
                size: proto_count,
                offset: proto_ids_off,
            });
        }

        let field_ids_off = self.pos();
        let field_count = plan.field_count() as u32;
        for f in plan.fields() {
            self.write_u16(f.class.0 as u16);
            self.write_u16(f.type_.0 as u16);
            self.write_u32(f.name.0);
        }
        if field_count > 0 {
            self.map_entries.push(MapItem {
                type_code: TYPE_FIELD_ID_ITEM,
                size: field_count,
                offset: field_ids_off,
            });
        }

        let method_ids_off = self.pos();
        let method_count = plan.method_count() as u32;
        for m in plan.methods() {
            self.write_u16(m.class.0 as u16);
            self.write_u16(u16::try_from(m.proto.0).map_err(|_| {
                crate::error::invalid("method id", "prototype index exceeds final pool width")
            })?);
            self.write_u32(m.name.0);
        }
        if method_count > 0 {
            self.map_entries.push(MapItem {
                type_code: TYPE_METHOD_ID_ITEM,
                size: method_count,
                offset: method_ids_off,
            });
        }

        let class_defs_off = self.pos();
        let class_count = plan.classes.len() as u32;
        for _ in 0..class_count {
            self.write_zeros(32);
        }
        if class_count > 0 {
            self.map_entries.push(MapItem {
                type_code: TYPE_CLASS_DEF_ITEM,
                size: class_count,
                offset: class_defs_off,
            });
        }

        let call_sites_off = self.write_optional_ids(plan)?;
        Ok(finalize::IdOffsets {
            strings: string_ids_off,
            types: type_ids_off,
            prototypes: proto_ids_off,
            fields: field_ids_off,
            methods: method_ids_off,
            classes: class_defs_off,
            call_sites: call_sites_off,
        })
    }

    fn write_optional_ids(&mut self, plan: &WritePlan<'_>) -> Result<Option<u32>> {
        let call_sites_off = if plan.call_site_count() == 0 {
            None
        } else {
            let off = self.pos();
            for _ in 0..plan.call_site_count() {
                self.write_u32(0);
            }
            self.map_entries.push(MapItem {
                type_code: TYPE_CALL_SITE_ID_ITEM,
                size: plan.call_site_count() as u32,
                offset: off,
            });
            Some(off)
        };

        if plan.method_handle_count() != 0 {
            let off = self.pos();
            for mh in plan.method_handles() {
                let mh = mh?;
                self.write_u16(mh.handle_type.to_u16());
                self.write_u16(0);
                let member_id = match &mh.member {
                    crate::types::method_handle::MethodHandleMember::Field(f) => u16::try_from(f.0)
                        .map_err(|_| {
                            crate::error::invalid(
                                "method_handle_item",
                                format!("field index {} exceeds u16 limit", f.0),
                            )
                        })?,
                    crate::types::method_handle::MethodHandleMember::Method(m) => {
                        u16::try_from(m.0).map_err(|_| {
                            crate::error::invalid(
                                "method_handle_item",
                                format!("method index {} exceeds u16 limit", m.0),
                            )
                        })?
                    }
                };
                self.write_u16(member_id);
                self.write_u16(0);
            }
            self.map_entries.push(MapItem {
                type_code: TYPE_METHOD_HANDLE_ITEM,
                size: plan.method_handle_count() as u32,
                offset: off,
            });
        }

        Ok(call_sites_off)
    }
}
