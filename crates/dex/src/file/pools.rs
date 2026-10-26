// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::borrow::Cow;

use super::{ClassTable, DexFile, HiddenApiData, IdTable, StringPool};
use crate::error::Result;
use crate::model::method_handle::{CallSiteItem, MethodHandle};
use crate::model::{
    CallSiteIdx, FieldId, FieldIdx, MethodHandleIdx, MethodId, MethodIdx, PerPool, Pool, ProtoIdx,
    Prototype, StringIdx, TypeIdx,
};

impl DexFile {
    pub fn strings(&self) -> &StringPool {
        &self.strings
    }

    pub fn types(&self) -> &IdTable<StringIdx> {
        &self.types
    }

    pub fn prototypes(&self) -> &IdTable<Prototype> {
        &self.prototypes
    }

    pub fn fields(&self) -> &IdTable<FieldId> {
        &self.fields
    }

    pub fn methods(&self) -> &IdTable<MethodId> {
        &self.methods
    }

    pub fn classes(&self) -> &ClassTable {
        &self.classes
    }

    /// Marks the DEX changed before granting mutation.
    pub fn classes_mut(&mut self) -> &mut ClassTable {
        self.touch();
        // The index locates methods by class position; insertion or removal can shift those.
        self.ref_index.take();
        &mut self.classes
    }

    pub fn call_sites(&self) -> impl ExactSizeIterator<Item = Result<Cow<'_, CallSiteItem>>> {
        self.call_sites.iter()
    }

    pub fn call_site(&self, idx: CallSiteIdx) -> Result<Cow<'_, CallSiteItem>> {
        self.call_sites.get(idx.0 as usize)
    }

    pub fn method_handles(&self) -> impl ExactSizeIterator<Item = Result<MethodHandle>> {
        self.method_handles
            .iter()
            .map(|handle| handle.map(Cow::into_owned))
    }

    pub fn method_handle(&self, idx: MethodHandleIdx) -> Result<MethodHandle> {
        self.method_handles.get(idx.0 as usize).map(Cow::into_owned)
    }

    pub fn hidden_api(&self) -> Option<&HiddenApiData> {
        self.hidden_api.as_ref()
    }

    pub fn type_string(&self, idx: TypeIdx) -> StringIdx {
        self.types.get(idx.0 as usize)
    }

    pub fn proto(&self, idx: ProtoIdx) -> Prototype {
        self.prototypes.get(idx.0 as usize)
    }

    pub fn field_id(&self, idx: FieldIdx) -> FieldId {
        self.fields.get(idx.0 as usize)
    }

    pub fn method_id(&self, idx: MethodIdx) -> MethodId {
        self.methods.get(idx.0 as usize)
    }

    pub(crate) fn pool_len(&self, pool: Pool) -> usize {
        match pool {
            Pool::String => self.strings.len(),
            Pool::Type => self.types.len(),
            Pool::Proto => self.prototypes.len(),
            Pool::Field => self.fields.len(),
            Pool::Method => self.methods.len(),
            Pool::CallSite => self.call_sites.len(),
            Pool::MethodHandle => self.method_handles.len(),
        }
    }

    /// Drops the entries appended to each pool since it held `lens[pool]`.
    pub(super) fn truncate_pools(&mut self, lens: &PerPool<usize>) {
        self.strings.truncate(lens[Pool::String]);
        self.types.truncate(lens[Pool::Type]);
        self.prototypes.truncate(lens[Pool::Proto]);
        self.fields.truncate(lens[Pool::Field]);
        self.methods.truncate(lens[Pool::Method]);
        self.call_sites.truncate(lens[Pool::CallSite]);
        self.method_handles.truncate(lens[Pool::MethodHandle]);
    }
}
