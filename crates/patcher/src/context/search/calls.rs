// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{HashMap, HashSet};

use reseam_dex::{DexFile, Instruction, MethodIdx, ProtoIdx, RefKey, StringIdx, TypeIdx};

use super::super::{InstructionLocation, MethodLocation, PatchContext};
use super::{MethodRefQuery, resolve, sites_of};
use crate::error::Result;

/// A method reference query resolved against the pools of one DEX.
struct MethodMatcher<'m, 'a> {
    context: &'m PatchContext<'a>,
    dex: &'m DexFile,
    query: &'m MethodRefQuery<'m>,
    owner: Option<TypeIdx>,
    name: Option<StringIdx>,
    protos: HashMap<ProtoIdx, bool>,
    owners: HashMap<TypeIdx, bool>,
}

impl<'m, 'a> MethodMatcher<'m, 'a> {
    /// `None` when the DEX lacks the owner or name, so none of its references match.
    fn new(
        context: &'m PatchContext<'a>,
        dex: &'m DexFile,
        query: &'m MethodRefQuery<'m>,
    ) -> Option<Self> {
        let (Ok(owner), Ok(name)) = (
            resolve(query.owner, |v| dex.find_type_idx(v)),
            resolve(query.name, |v| dex.strings().find(v)),
        ) else {
            return None;
        };
        Some(Self {
            context,
            dex,
            query,
            owner,
            name,
            protos: HashMap::new(),
            owners: HashMap::new(),
        })
    }

    fn accepts(&mut self, method: MethodIdx) -> bool {
        let id = self.dex.method_id(method);
        self.owner.is_none_or(|owner| id.class == owner)
            && self.name.is_none_or(|name| id.name == name)
            && self.accepts_proto(id.proto)
            && self.accepts_owner(id.class)
    }

    fn accepts_proto(&mut self, proto: ProtoIdx) -> bool {
        let (dex, query) = (self.dex, self.query);
        *self.protos.entry(proto).or_insert_with(|| {
            let proto = dex.proto(proto);
            let parameter = |t: &TypeIdx| dex.type_descriptor(*t);
            query
                .return_type
                .is_none_or(|v| dex.type_descriptor(proto.return_type) == v)
                && query.parameters.is_none_or(|v| {
                    v.len() == proto.parameters.len()
                        && v.iter()
                            .zip(&proto.parameters)
                            .all(|(v, t)| *v == parameter(t))
                })
                && query
                    .parameter_count
                    .is_none_or(|count| count == proto.parameters.len())
                && query
                    .required_parameters
                    .iter()
                    .all(|v| proto.parameters.iter().any(|t| *v == parameter(t)))
        })
    }

    fn accepts_owner(&mut self, class: TypeIdx) -> bool {
        let (context, dex, query) = (self.context, self.dex, self.query);
        *self.owners.entry(class).or_insert_with(|| {
            let owner = dex.type_descriptor(class);
            query
                .owner_assignable_to
                .iter()
                .all(|expected| context.is_assignable(&owner, expected))
        })
    }

    /// The method ids of the DEX the query accepts.
    fn members(&mut self) -> HashSet<MethodIdx> {
        let candidates: Vec<MethodIdx> = match self.owner {
            Some(owner) => self.dex.methods_of(owner, self.name).collect(),
            None => (0..self.dex.methods().len() as u32)
                .map(MethodIdx)
                .collect(),
        };
        candidates
            .into_iter()
            .filter(|&method| self.accepts(method))
            .collect()
    }
}

impl PatchContext<'_> {
    /// Calls matching `query` across the app. Referenced method ids are matched before their
    /// indexed call sites are scanned, so platform methods the APK does not define match too.
    pub fn find_calls_matching(
        &self,
        query: &MethodRefQuery<'_>,
    ) -> Result<Vec<InstructionLocation>> {
        self.scan_all("calls matching reference", |dex_idx, dex| {
            let members = MethodMatcher::new(self, dex, query)
                .map(|mut matcher| matcher.members())
                .unwrap_or_default();
            sites_of(dex_idx, dex, &members, RefKey::Method, |site| {
                site.instruction.method_ref()
            })
        })
    }

    /// The instructions of `method` calling a method `query` matches.
    pub fn find_calls_in(
        &mut self,
        method: MethodLocation,
        query: &MethodRefQuery<'_>,
    ) -> Result<Vec<usize>> {
        let references = self.references_in(method, Instruction::method_ref)?;
        let Some(mut matcher) = self
            .dex_file(method.dex_idx)
            .and_then(|dex| MethodMatcher::new(self, dex, query))
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
