// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::sort::RemapTables;
use super::DexWriter;
use super::{annotations, class_data, code, encoded_arrays, finalize};
use crate::encoding::leb128::write_uleb128;
use crate::encoding::mutf8::{encode_mutf8, utf16_len};
use crate::error::Result;
use crate::file::DexFile;
use crate::types::map::{
    MapItem, TYPE_CALL_SITE_ID_ITEM, TYPE_CLASS_DEF_ITEM, TYPE_FIELD_ID_ITEM, TYPE_HEADER_ITEM,
    TYPE_MAP_LIST, TYPE_METHOD_HANDLE_ITEM, TYPE_METHOD_ID_ITEM, TYPE_PROTO_ID_ITEM,
    TYPE_STRING_DATA_ITEM, TYPE_STRING_ID_ITEM, TYPE_TYPE_ID_ITEM,
};

/// Writes all sections of a DEX file in spec order, then backpatches offsets.
pub(crate) trait DexWriterWriteExt {
    fn write_dex(&mut self, dex: &DexFile, remap: Option<&RemapTables>) -> Result<()>;
}

impl DexWriterWriteExt for DexWriter {
    fn write_dex(&mut self, dex: &DexFile, remap: Option<&RemapTables>) -> Result<()> {
        let header_off = self.pos();
        self.header_base = header_off;
        let header_size = dex.required_version().header_size() as usize;
        self.buf.extend_from_slice(&vec![0u8; header_size]);
        self.map_entries.clear();
        self.map_entries.push(MapItem {
            type_code: TYPE_HEADER_ITEM,
            size: 1,
            offset: header_off,
        });

        let string_ids_off = self.pos();
        let string_count = dex.strings.len() as u32;
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
        let type_count = dex.types.len() as u32;
        for desc_idx in &dex.types {
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
        let proto_count = dex.prototypes.len() as u32;
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
        let field_count = dex.fields.len() as u32;
        for f in &dex.fields {
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
        let method_count = dex.methods.len() as u32;
        for m in &dex.methods {
            self.write_u16(m.class.0 as u16);
            self.write_u16(m.proto.0);
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
        let class_count = dex.classes.len() as u32;
        for _ in 0..class_count {
            self.buf.extend_from_slice(&[0u8; 32]);
        }
        if class_count > 0 {
            self.map_entries.push(MapItem {
                type_code: TYPE_CLASS_DEF_ITEM,
                size: class_count,
                offset: class_defs_off,
            });
        }

        let call_sites_off = if !dex.call_sites.is_empty() {
            let off = self.pos();
            for _ in 0..dex.call_sites.len() {
                self.write_u32(0);
            }
            self.map_entries.push(MapItem {
                type_code: TYPE_CALL_SITE_ID_ITEM,
                size: dex.call_sites.len() as u32,
                offset: off,
            });
            Some(off)
        } else {
            None
        };

        if !dex.method_handles.is_empty() {
            let off = self.pos();
            for mh in &dex.method_handles {
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
                size: dex.method_handles.len() as u32,
                offset: off,
            });
        }

        let data_off = self.pos();

        let (proto_param_offsets, class_interface_offsets) =
            encoded_arrays::write_type_lists(self, dex);

        let class_ann_datas = annotations::write_annotations(self, dex);

        let layouts = code::write_code_and_debug(self, dex, remap)?;
        class_data::write_class_data_items(self, &layouts);

        let string_data_start = self.pos();
        self.string_data_offsets.clear();
        for s in &dex.strings {
            let off = self.pos();
            self.string_data_offsets.push(off);
            let mutf8 = encode_mutf8(s.as_str());
            let utf16_size = utf16_len(s.as_str());
            write_uleb128(&mut self.buf, utf16_size);
            self.buf.extend_from_slice(&mutf8);
            self.buf.push(0);
        }
        if !dex.strings.is_empty() {
            self.map_entries.push(MapItem {
                type_code: TYPE_STRING_DATA_ITEM,
                size: dex.strings.len() as u32,
                offset: string_data_start,
            });
        }

        let (static_values_offsets, call_site_data_offsets) =
            encoded_arrays::write_encoded_arrays(self, dex);

        if let Some(ref hidden_api) = dex.hidden_api {
            self.align(4);
            let hidden_api_off = self.pos();
            encoded_arrays::write_hidden_api(self, hidden_api, dex);
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

        let data_size = self.pos() - data_off;

        finalize::finalize(
            self,
            dex,
            data_off,
            data_size,
            string_ids_off,
            type_ids_off,
            proto_ids_off,
            field_ids_off,
            method_ids_off,
            class_defs_off,
            map_off,
            &class_ann_datas,
            &proto_param_offsets,
            &class_interface_offsets,
            &static_values_offsets,
            &call_site_data_offsets,
            call_sites_off,
        )
    }
}
