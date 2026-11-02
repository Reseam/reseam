// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod classes;
mod members;
mod values;

use std::collections::BTreeMap;

use reseam_dex::{DexFile, InstructionRef, RefKey, RefQuery, StringIdx};
use reseam_model::analysis::{
    CodeSite, Member, MethodRef, RefKind, RefSite, RefTarget, References,
};

use super::Analysis;
use super::code::{body, named_member, ref_kind, scan, sites};
use crate::error::{HostError, Result};

/// The test a search puts each instruction to: what it does with the target,
/// and the member it names.
pub(super) trait Matches:
    Fn(&DexFile, InstructionRef<'_>) -> Option<(RefKind, Option<Member>)> + Sync
{
}

impl<F> Matches for F where
    F: Fn(&DexFile, InstructionRef<'_>) -> Option<(RefKind, Option<Member>)> + Sync
{
}

/// What one DEX searches for: the index query that narrows the methods to
/// read, and the test each instruction must pass.
pub(super) struct Search<F> {
    pub(super) query: RefQuery,
    pub(super) matches: F,
}

impl Analysis {
    /// Every reference to `target`: the instructions that use it, and the
    /// declarations and XML that relate to it.
    pub fn refs(&mut self, target: &RefTarget) -> Result<References> {
        let mut references = match target {
            RefTarget::Member(member) => self.member_refs(member)?,
            RefTarget::String(value) => References {
                sites: self.string_sites(value)?,
                relations: Vec::new(),
            },
            RefTarget::Number(value) => References {
                sites: self.number_sites(*value)?,
                relations: Vec::new(),
            },
            RefTarget::Resource(id) => self.resource_refs(*id)?,
            RefTarget::Package(package) => References {
                sites: self.package_sites(package)?,
                relations: Vec::new(),
            },
        };
        references.sites.sort();
        references.relations.sort();
        Ok(references)
    }

    /// The references to `member`; it is unknown when nothing references it
    /// and the APK does not define its class.
    fn member_refs(&self, member: &Member) -> Result<References> {
        let (references, class) = match member {
            Member::Field(field) => (self.field_refs(field)?, &field.class),
            Member::Method(method) => (self.method_refs(method)?, &method.class),
            Member::Class(class) => (self.class_refs(class)?, class),
        };
        let unknown = references.sites.is_empty()
            && references.relations.is_empty()
            && self.hierarchy().get(class).is_none();
        if unknown {
            return Err(HostError::UnknownMember(member.clone()));
        }
        Ok(references)
    }

    /// What `method` references: calls, field accesses, types, strings and numbers.
    pub fn refs_from(&self, method: &MethodRef) -> Result<Vec<RefSite>> {
        let (dex, class, idx) = self.method(method)?;
        let Some(code) = body(&class, idx) else {
            return Ok(Vec::new());
        };
        let class_idx = self
            .hierarchy()
            .get(&method.class)
            .expect("the method's class was just found")
            .index;
        let hits = code
            .instructions()
            .iter()
            .enumerate()
            .filter_map(|(index, instruction)| {
                let kind = ref_kind(instruction.opcode()?)?;
                let target = named_member(dex, instruction);
                Some((u32::try_from(index).ok()?, (kind, target)))
            })
            .collect();
        let hits = vec![super::code::MethodHits {
            class_idx,
            method: idx,
            hits,
        }];
        Ok(ref_sites(sites(dex, hits)?))
    }

    /// The sites each DEX's search finds.
    pub(super) fn search<F>(
        &self,
        search: impl Fn(&DexFile) -> Option<Search<F>>,
    ) -> Result<Vec<RefSite>>
    where
        F: Matches,
    {
        self.search_each(|_, dex| search(dex))
    }

    pub(super) fn search_each<F>(
        &self,
        search: impl Fn(usize, &DexFile) -> Option<Search<F>>,
    ) -> Result<Vec<RefSite>>
    where
        F: Matches,
    {
        let mut found = Vec::new();
        for (dex_idx, dex) in self.dex().iter().enumerate() {
            let Some(search) = search(dex_idx, dex) else {
                continue;
            };
            let matches = search.matches;
            let hits = scan(dex, &search.query, |instruction| matches(dex, instruction))?;
            found.extend(ref_sites(sites(dex, hits)?));
        }
        Ok(found)
    }
}

pub(super) fn string_search(string: StringIdx) -> Search<impl Matches> {
    Search {
        query: RefQuery::all_of([RefKey::String(string)]),
        matches: move |_: &DexFile, instruction: InstructionRef<'_>| {
            (instruction.string_ref()? == string).then_some((RefKind::LoadString, None))
        },
    }
}

fn ref_sites(sites: Vec<(CodeSite, (RefKind, Option<Member>))>) -> Vec<RefSite> {
    sites
        .into_iter()
        .map(|(site, (kind, target))| RefSite { site, kind, target })
        .collect()
}

/// The callers of a method, by caller, each with its first call site.
pub(super) fn callers(sites: Vec<RefSite>) -> BTreeMap<MethodRef, CodeSite> {
    let mut callers = BTreeMap::new();
    for site in sites {
        if site.kind == RefKind::Call {
            callers.entry(site.site.method.clone()).or_insert(site.site);
        }
    }
    callers
}
