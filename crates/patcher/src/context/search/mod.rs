// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod calls;
mod field_accesses;
mod instructions;
mod methods;

use std::collections::HashSet;
use std::hash::Hash;

use super::{InstructionLocation, MethodLocation, PatchContext};
use crate::error::Result;
use reseam_dex::{DexFile, Instruction, InstructionSite, MethodHit, RefKey, RefQuery};
use tracing::debug;

type DexResult<T> = reseam_dex::Result<T>;

impl PatchContext<'_> {
    /// The index and reference of each instruction of `method` that `reference` finds one in.
    fn references_in<I>(
        &mut self,
        method: MethodLocation,
        reference: impl Fn(&Instruction) -> Option<I>,
    ) -> Result<Vec<(usize, I)>> {
        let code = self
            .read_method(method)?
            .and_then(|(_, method)| method.code.as_deref());
        Ok(code.map_or_else(Vec::new, |code| {
            code.instructions()
                .iter()
                .enumerate()
                .filter_map(|(index, instruction)| Some((index, reference(instruction)?)))
                .collect()
        }))
    }

    pub(super) fn scan_all<T>(
        &self,
        what: &str,
        scan: impl Fn(usize, &DexFile) -> DexResult<Vec<T>>,
    ) -> Result<Vec<T>> {
        debug!(what, "full scan");
        self.dex()
            .iter()
            .enumerate()
            .try_fold(Vec::new(), |mut hits, (index, dex)| {
                hits.extend(scan(index, dex)?);
                Ok(hits)
            })
    }
}

/// A method reference to find calls to. Unset parts match anything;
/// `required_parameters` must each appear somewhere in the parameter list, and the
/// owner must be assignable to each of `owner_assignable_to`.
#[derive(Debug, Clone, Copy, Default)]
pub struct MethodRefQuery<'a> {
    pub owner: Option<&'a str>,
    pub name: Option<&'a str>,
    pub return_type: Option<&'a str>,
    pub parameters: Option<&'a [&'a str]>,
    pub required_parameters: &'a [&'a str],
    pub parameter_count: Option<usize>,
    pub owner_assignable_to: &'a [&'a str],
}

/// A field reference to find reads and writes of. Unset parts match anything.
pub struct FieldRefQuery<'a> {
    pub owner: Option<&'a str>,
    pub name: Option<&'a str>,
    pub field_type: Option<&'a str>,
}

pub(super) struct Unresolved;

pub(super) fn resolve<T>(
    value: Option<&str>,
    find: impl FnOnce(&str) -> Option<T>,
) -> std::result::Result<Option<T>, Unresolved> {
    value.map_or(Ok(None), |value| find(value).map(Some).ok_or(Unresolved))
}

pub(super) fn method_location(dex_idx: usize, hit: &MethodHit) -> MethodLocation {
    MethodLocation {
        dex_idx,
        class_idx: hit.class_idx,
        method_idx: hit.method_pos,
        kind: hit.kind,
    }
}

pub(super) fn instruction_location(
    dex_idx: usize,
    site: &InstructionSite<'_>,
) -> InstructionLocation {
    InstructionLocation {
        method: MethodLocation {
            dex_idx,
            class_idx: site.class_idx,
            method_idx: site.method_pos,
            kind: site.kind,
        },
        insn_idx: site.insn_idx,
    }
}

/// The instructions of `dex` whose reference is one of `members`.
pub(super) fn sites_of<I: Copy + Eq + Hash + Sync>(
    dex_idx: usize,
    dex: &DexFile,
    members: &HashSet<I>,
    key: impl Fn(I) -> RefKey,
    reference: impl Fn(&InstructionSite<'_>) -> Option<I> + Sync,
) -> reseam_dex::Result<Vec<InstructionLocation>> {
    if members.is_empty() {
        return Ok(Vec::new());
    }
    let query = RefQuery::any_of(members.iter().copied().map(key));
    dex.scan_instructions(&query, |site| {
        members
            .contains(&reference(site)?)
            .then(|| instruction_location(dex_idx, site))
    })
}
