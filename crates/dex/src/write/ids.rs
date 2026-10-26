// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::DexWriter;
use super::finalize::IdOffsets;
use super::plan::WritePlan;
use super::sink::DexSink;
use crate::error::Result;
use crate::model::map::{
    MapItem, TYPE_CALL_SITE_ID_ITEM, TYPE_CLASS_DEF_ITEM, TYPE_FIELD_ID_ITEM,
    TYPE_METHOD_HANDLE_ITEM, TYPE_METHOD_ID_ITEM, TYPE_PROTO_ID_ITEM, TYPE_STRING_ID_ITEM,
    TYPE_TYPE_ID_ITEM,
};

impl<S: DexSink> DexWriter<S> {
    pub(super) fn write_ids(&mut self, plan: &WritePlan<'_>) -> Result<IdOffsets> {
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
        Ok(IdOffsets {
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
                    crate::model::method_handle::MethodHandleMember::Field(f) => u16::try_from(f.0)
                        .map_err(|_| {
                            crate::error::invalid(
                                "method_handle_item",
                                format!("field index {} exceeds u16 limit", f.0),
                            )
                        })?,
                    crate::model::method_handle::MethodHandleMember::Method(m) => {
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
