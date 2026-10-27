// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::DexWriter;
use super::plan::WritePlan;
use super::sink::DexSink;
use super::{annotations, class_data, code, encoded_arrays, finalize, hidden_api, type_lists};
use crate::error::Result;
use crate::model::Pool;
use crate::model::map::{
    TYPE_HEADER_ITEM, TYPE_HIDDENAPI_CLASS_DATA_ITEM, TYPE_MAP_LIST, TYPE_STRING_DATA_ITEM,
};

impl<S: DexSink> DexWriter<S> {
    pub(crate) fn write_dex(
        &mut self,
        plan: &WritePlan<'_>,
        version: crate::model::header::DexVersion,
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
        self.map_section(TYPE_HEADER_ITEM, header_off, 1);

        let ids = self.write_ids(plan)?;
        let data_off = self.pos();

        let (proto_param_offsets, class_interface_offsets) =
            type_lists::write_type_lists(self, plan)?;

        let class_ann_datas = annotations::write_annotations(self, plan)?;

        let layouts = code::write_code_and_debug(self, plan)?;
        dex.release_pages();
        class_data::write_class_data_items(self, &layouts);

        let string_data_start = self.pos();
        self.string_data_offsets.clear();
        for i in 0..plan.count(Pool::String) {
            let off = self.pos();
            self.string_data_offsets.push(off);
            self.write(&plan.string_item(i));
        }
        dex.release_pages();
        self.map_section(
            TYPE_STRING_DATA_ITEM,
            string_data_start,
            plan.count(Pool::String) as u32,
        );

        let (static_values_offsets, call_site_data_offsets) =
            encoded_arrays::write_encoded_arrays(self, plan)?;

        if let Some(ref hidden_api) = dex.hidden_api {
            self.align(4);
            let hidden_api_off = self.pos();
            hidden_api::write_hidden_api(self, hidden_api, plan)?;
            self.map_section(TYPE_HIDDENAPI_CLASS_DATA_ITEM, hidden_api_off, 1);
        }

        self.align(4);
        let map_off = self.pos();
        self.map_section(TYPE_MAP_LIST, map_off, 1);
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
}
