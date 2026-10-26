// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::borrow::Cow;

use super::WritePlan;
use crate::error::Result;
use crate::model::method_handle::{CallSiteItem, MethodHandle};
use crate::model::{FieldId, MethodId, Prototype, StringIdx, TypeIdx};

impl WritePlan<'_> {
    pub(crate) fn string_count(&self) -> usize {
        self.order
            .as_ref()
            .map_or(self.dex.strings.len(), |o| o.string.len())
    }

    pub(crate) fn type_count(&self) -> usize {
        self.order
            .as_ref()
            .map_or(self.dex.types.len(), |o| o.type_.len())
    }

    pub(crate) fn proto_count(&self) -> usize {
        self.order
            .as_ref()
            .map_or(self.dex.prototypes.len(), |o| o.proto.len())
    }

    pub(crate) fn field_count(&self) -> usize {
        self.order
            .as_ref()
            .map_or(self.dex.fields.len(), |o| o.field.len())
    }

    pub(crate) fn method_count(&self) -> usize {
        self.order
            .as_ref()
            .map_or(self.dex.methods.len(), |o| o.method.len())
    }

    pub(crate) fn call_site_count(&self) -> usize {
        self.order
            .as_ref()
            .map_or(self.dex.call_sites.len(), |o| o.call_site.len())
    }

    pub(crate) fn method_handle_count(&self) -> usize {
        self.order
            .as_ref()
            .map_or(self.dex.method_handles.len(), |o| o.method_handle.len())
    }

    pub(crate) fn string_item(&self, new: usize) -> Cow<'_, [u8]> {
        let old = self.order.as_ref().map_or(new as u32, |o| o.string[new]);
        self.dex.strings.item(StringIdx(old))
    }

    pub(crate) fn types(&self) -> impl Iterator<Item = StringIdx> + '_ {
        (0..self.type_count()).map(move |new| {
            let old = self.order.as_ref().map_or(new as u32, |o| o.type_[new]);
            self.map_string(self.dex.types.get(old as usize))
        })
    }

    pub(crate) fn prototypes(&self) -> impl Iterator<Item = Prototype> + '_ {
        (0..self.proto_count()).map(move |new| {
            let old = self.order.as_ref().map_or(new as u32, |o| o.proto[new]);
            let p = self.dex.prototypes.get(old as usize);
            Prototype {
                shorty: self.map_string(p.shorty),
                return_type: self.map_type(p.return_type),
                parameters: p.parameters.iter().map(|t| self.map_type(*t)).collect(),
            }
        })
    }

    pub(crate) fn fields(&self) -> impl Iterator<Item = FieldId> + '_ {
        (0..self.field_count()).map(move |new| {
            let old = self.order.as_ref().map_or(new as u32, |o| o.field[new]);
            let f = self.dex.fields.get(old as usize);
            FieldId {
                class: self.map_type(f.class),
                type_: self.map_type(f.type_),
                name: self.map_string(f.name),
            }
        })
    }

    pub(crate) fn methods(&self) -> impl Iterator<Item = MethodId> + '_ {
        (0..self.method_count()).map(move |new| {
            let old = self.order.as_ref().map_or(new as u32, |o| o.method[new]);
            let m = self.dex.methods.get(old as usize);
            MethodId {
                class: self.map_type(m.class),
                proto: self.map_proto(m.proto),
                name: self.map_string(m.name),
            }
        })
    }

    pub(crate) fn call_sites(&self) -> impl Iterator<Item = Result<Cow<'_, CallSiteItem>>> {
        (0..self.call_site_count()).map(|new| {
            let old = self
                .order
                .as_ref()
                .map_or(new as u32, |order| order.call_site[new]);
            let mut site = self.dex.call_sites.get(old as usize)?;
            if let Some(remap) = self.remap() {
                remap.remap_call_site(site.to_mut())?;
            }
            Ok(site)
        })
    }
    pub(crate) fn method_handles(&self) -> impl Iterator<Item = Result<MethodHandle>> + '_ {
        (0..self.method_handle_count()).map(|new| {
            let old = self
                .order
                .as_ref()
                .map_or(new as u32, |order| order.method_handle[new]);
            let mut handle = *self.dex.method_handles.get(old as usize)?;
            if let Some(remap) = self.remap() {
                remap.remap_method_handle(&mut handle)?;
            }
            Ok(handle)
        })
    }

    pub(super) fn map_string(&self, idx: StringIdx) -> StringIdx {
        match &self.remap {
            Some(r) => StringIdx(r.string[idx.0 as usize]),
            None => idx,
        }
    }

    pub(super) fn map_type(&self, idx: TypeIdx) -> TypeIdx {
        match &self.remap {
            Some(r) => TypeIdx(r.type_[idx.0 as usize]),
            None => idx,
        }
    }

    pub(super) fn map_proto(&self, idx: crate::model::ProtoIdx) -> crate::model::ProtoIdx {
        match &self.remap {
            Some(r) => crate::model::ProtoIdx(r.proto[idx.0 as usize]),
            None => idx,
        }
    }
}
