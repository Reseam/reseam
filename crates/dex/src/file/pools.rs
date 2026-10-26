// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{ClassTable, DexFile, FileTable, HiddenApiData, IdTable, StringPool};
use crate::types::method_handle::{CallSiteItem, MethodHandle};
use crate::types::{
    FieldId, FieldIdx, MethodId, MethodIdx, ProtoIdx, Prototype, StringIdx, TypeIdx,
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

    pub fn call_sites(&self) -> &FileTable<CallSiteItem> {
        &self.call_sites
    }

    /// Marks the DEX changed before granting mutation.
    pub fn call_sites_mut(&mut self) -> &mut FileTable<CallSiteItem> {
        self.touch();
        &mut self.call_sites
    }

    pub fn method_handles(&self) -> &FileTable<MethodHandle> {
        &self.method_handles
    }

    /// Marks the DEX changed before granting mutation.
    pub fn method_handles_mut(&mut self) -> &mut FileTable<MethodHandle> {
        self.touch();
        &mut self.method_handles
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
}
