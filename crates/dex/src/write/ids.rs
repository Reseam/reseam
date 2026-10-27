// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::DexWriter;
use super::finalize::IdOffsets;
use super::plan::WritePlan;
use super::sink::DexSink;
use crate::error::Result;
use crate::model::Pool;
use crate::model::map::{
    TYPE_CALL_SITE_ID_ITEM, TYPE_CLASS_DEF_ITEM, TYPE_FIELD_ID_ITEM, TYPE_METHOD_HANDLE_ITEM,
    TYPE_METHOD_ID_ITEM, TYPE_PROTO_ID_ITEM, TYPE_STRING_ID_ITEM, TYPE_TYPE_ID_ITEM,
};
use crate::model::method_handle::MethodHandleMember;

impl<S: DexSink> DexWriter<S> {
    /// Writes the id sections. Tables that point into the data section are
    /// zeroed here and patched once the data is laid out. Every pool fits
    /// the 16-bit indices these tables use, which the write plan checked.
    pub(super) fn write_ids(&mut self, plan: &WritePlan<'_>) -> Result<IdOffsets> {
        let strings = plan.count(Pool::String);
        let strings = self.section(TYPE_STRING_ID_ITEM, strings, |w| w.write_zeros(strings * 4));
        let types = self.section(TYPE_TYPE_ID_ITEM, plan.count(Pool::Type), |w| {
            for descriptor in plan.types() {
                w.write_u32(descriptor.0);
            }
        });
        let protos = plan.count(Pool::Proto);
        let prototypes = self.section(TYPE_PROTO_ID_ITEM, protos, |w| w.write_zeros(protos * 12));
        let fields = self.section(TYPE_FIELD_ID_ITEM, plan.count(Pool::Field), |w| {
            for field in plan.fields() {
                w.write_u16(field.class.0 as u16);
                w.write_u16(field.type_.0 as u16);
                w.write_u32(field.name.0);
            }
        });
        let methods = self.section(TYPE_METHOD_ID_ITEM, plan.count(Pool::Method), |w| {
            for method in plan.methods() {
                w.write_u16(method.class.0 as u16);
                w.write_u16(method.proto.0 as u16);
                w.write_u32(method.name.0);
            }
        });
        let classes = plan.classes.len();
        let classes = self.section(TYPE_CLASS_DEF_ITEM, classes, |w| {
            w.write_zeros(classes * 32);
        });
        let call_sites = plan.count(Pool::CallSite);
        let call_sites = (call_sites != 0).then(|| {
            self.section(TYPE_CALL_SITE_ID_ITEM, call_sites, |w| {
                w.write_zeros(call_sites * 4);
            })
        });
        let handles = plan.method_handles().collect::<Result<Vec<_>>>()?;
        self.section(TYPE_METHOD_HANDLE_ITEM, handles.len(), |w| {
            for handle in &handles {
                let member = match handle.member {
                    MethodHandleMember::Field(field) => field.0,
                    MethodHandleMember::Method(method) => method.0,
                };
                w.write_u16(handle.handle_type.to_u16());
                w.write_u16(0);
                w.write_u16(member as u16);
                w.write_u16(0);
            }
        });
        Ok(IdOffsets {
            strings,
            types,
            prototypes,
            fields,
            methods,
            classes,
            call_sites,
        })
    }
}
