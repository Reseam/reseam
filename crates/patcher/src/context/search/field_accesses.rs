// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use reseam_dex::{DexFile, FieldIdx, Instruction, RefKey, StringIdx, TypeIdx};

use super::super::{InstructionLocation, MethodLocation, PatchContext};
use super::{FieldRefQuery, resolve, sites_of};
use crate::error::Result;

/// A field reference query resolved against the pools of one DEX.
struct FieldMatcher<'m> {
    dex: &'m DexFile,
    owner: Option<TypeIdx>,
    name: Option<StringIdx>,
    field_type: Option<TypeIdx>,
}

impl<'m> FieldMatcher<'m> {
    /// `None` when the DEX lacks a named part, so none of its references match.
    fn new(dex: &'m DexFile, query: &FieldRefQuery<'_>) -> Option<Self> {
        let (Ok(owner), Ok(name), Ok(field_type)) = (
            resolve(query.owner, |v| dex.find_type_idx(v)),
            resolve(query.name, |v| dex.strings().find(v)),
            resolve(query.field_type, |v| dex.find_type_idx(v)),
        ) else {
            return None;
        };
        Some(Self {
            dex,
            owner,
            name,
            field_type,
        })
    }

    fn accepts(&self, field: FieldIdx) -> bool {
        let id = self.dex.field_id(field);
        self.owner.is_none_or(|owner| id.class == owner)
            && self.name.is_none_or(|name| id.name == name)
            && self
                .field_type
                .is_none_or(|field_type| id.type_ == field_type)
    }

    fn members(&self) -> HashSet<FieldIdx> {
        match self.owner {
            Some(owner) => self
                .dex
                .fields_of(owner, self.name)
                .filter(|&field| self.accepts(field))
                .collect(),
            None => (0..self.dex.fields().len() as u32)
                .map(FieldIdx)
                .filter(|&field| self.accepts(field))
                .collect(),
        }
    }
}

impl PatchContext<'_> {
    /// Reads and writes of fields matching `query` across the app.
    pub fn find_field_accesses_matching(
        &self,
        query: &FieldRefQuery<'_>,
    ) -> Result<Vec<InstructionLocation>> {
        self.scan_all("field accesses matching reference", |dex_idx, dex| {
            let members = FieldMatcher::new(dex, query)
                .map(|matcher| matcher.members())
                .unwrap_or_default();
            sites_of(dex_idx, dex, &members, RefKey::Field, |site| {
                site.instruction.field_ref()
            })
        })
    }

    /// The instructions of `method` reading or writing a field `query` matches.
    pub fn find_field_accesses_in(
        &mut self,
        method: MethodLocation,
        query: &FieldRefQuery<'_>,
    ) -> Result<Vec<usize>> {
        let references = self.references_in(method, Instruction::field_ref)?;
        let Some(matcher) = self
            .dex_file(method.dex_idx)
            .and_then(|dex| FieldMatcher::new(dex, query))
        else {
            return Ok(Vec::new());
        };
        Ok(references
            .into_iter()
            .filter(|&(_, reference)| matcher.accepts(reference))
            .map(|(index, _)| index)
            .collect())
    }
}
