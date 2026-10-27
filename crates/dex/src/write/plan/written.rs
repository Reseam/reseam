// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::borrow::Cow;

use super::WritePlan;
use crate::error::Result;
use crate::model::method_handle::{CallSiteItem, MethodHandle};
use crate::model::{FieldId, MethodId, Pool, ProtoIdx, Prototype, StringIdx, TypeIdx};

impl WritePlan<'_> {
    /// How many entries the written `pool` holds.
    pub(crate) fn count(&self, pool: Pool) -> usize {
        self.order
            .as_ref()
            .map_or_else(|| self.dex.pool_len(pool), |order| order[pool].len())
    }

    /// The source index of written entry `new` of `pool`.
    fn source(&self, pool: Pool, new: usize) -> usize {
        self.order
            .as_ref()
            .map_or(new, |order| order[pool][new] as usize)
    }

    pub(crate) fn string_item(&self, new: usize) -> Cow<'_, [u8]> {
        self.dex
            .strings
            .item(StringIdx(self.source(Pool::String, new) as u32))
    }

    pub(crate) fn types(&self) -> impl Iterator<Item = StringIdx> + '_ {
        (0..self.count(Pool::Type))
            .map(move |new| self.map_string(self.dex.types.get(self.source(Pool::Type, new))))
    }

    pub(crate) fn prototypes(&self) -> impl Iterator<Item = Prototype> + '_ {
        (0..self.count(Pool::Proto)).map(move |new| {
            let p = self.dex.prototypes.get(self.source(Pool::Proto, new));
            Prototype {
                shorty: self.map_string(p.shorty),
                return_type: self.map_type(p.return_type),
                parameters: p.parameters.iter().map(|t| self.map_type(*t)).collect(),
            }
        })
    }

    pub(crate) fn fields(&self) -> impl Iterator<Item = FieldId> + '_ {
        (0..self.count(Pool::Field)).map(move |new| {
            let f = self.dex.fields.get(self.source(Pool::Field, new));
            FieldId {
                class: self.map_type(f.class),
                type_: self.map_type(f.type_),
                name: self.map_string(f.name),
            }
        })
    }

    pub(crate) fn methods(&self) -> impl Iterator<Item = MethodId> + '_ {
        (0..self.count(Pool::Method)).map(move |new| {
            let m = self.dex.methods.get(self.source(Pool::Method, new));
            MethodId {
                class: self.map_type(m.class),
                proto: self.map_proto(m.proto),
                name: self.map_string(m.name),
            }
        })
    }

    pub(crate) fn call_sites(&self) -> impl Iterator<Item = Result<Cow<'_, CallSiteItem>>> {
        (0..self.count(Pool::CallSite)).map(|new| {
            let mut site = self.dex.call_sites.get(self.source(Pool::CallSite, new))?;
            if let Some(remap) = self.remap() {
                remap.remap_call_site(site.to_mut())?;
            }
            Ok(site)
        })
    }

    pub(crate) fn method_handles(&self) -> impl Iterator<Item = Result<MethodHandle>> + '_ {
        (0..self.count(Pool::MethodHandle)).map(|new| {
            let mut handle = *self
                .dex
                .method_handles
                .get(self.source(Pool::MethodHandle, new))?;
            if let Some(remap) = self.remap() {
                remap.remap_method_handle(&mut handle)?;
            }
            Ok(handle)
        })
    }

    pub(super) fn map_string(&self, idx: StringIdx) -> StringIdx {
        self.remap().map_or(idx, |remap| remap.remap_string(idx))
    }

    pub(super) fn map_type(&self, idx: TypeIdx) -> TypeIdx {
        self.remap().map_or(idx, |remap| remap.remap_type(idx))
    }

    fn map_proto(&self, idx: ProtoIdx) -> ProtoIdx {
        self.remap().map_or(idx, |remap| remap.remap_proto(idx))
    }
}
