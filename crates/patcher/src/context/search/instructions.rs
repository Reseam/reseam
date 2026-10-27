// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use super::super::{InstructionLocation, PatchContext};
use super::{DexResult, instruction_location};
use crate::error::Result;
use reseam_dex::{DexFile, RefKey, RefQuery, StringIdx};

impl PatchContext<'_> {
    pub fn find_instructions_by_literal(&self, literal: i64) -> Result<Vec<InstructionLocation>> {
        self.scan_all("instructions by literal", |dex_idx, dex| {
            dex.scan_instructions(&RefQuery::all_of([RefKey::Literal(literal)]), |site| {
                (site.instruction.literal() == Some(literal))
                    .then(|| instruction_location(dex_idx, site))
            })
        })
    }

    pub fn find_instructions_by_string(&self, target: &str) -> Result<Vec<InstructionLocation>> {
        self.scan_all("instructions by string", |dex_idx, dex| {
            let Some(target_idx) = dex.strings().find(target) else {
                return Ok(Vec::new());
            };
            dex.scan_instructions(&RefQuery::all_of([RefKey::String(target_idx)]), |site| {
                (site.instruction.string_ref() == Some(target_idx))
                    .then(|| instruction_location(dex_idx, site))
            })
        })
    }

    pub fn find_instructions_by_string_contains(
        &self,
        substring: &str,
    ) -> Result<Vec<InstructionLocation>> {
        self.scan_all("instructions by substring", |dex_idx, dex| {
            let matches = dex
                .strings()
                .iter()
                .enumerate()
                .filter(|(_, s)| s.contains(substring))
                .map(|(i, _)| StringIdx(i as u32))
                .collect();
            string_loads(dex_idx, dex, &matches)
        })
    }

    pub fn find_instructions_by_string_prefix(
        &self,
        prefix: &str,
    ) -> Result<Vec<InstructionLocation>> {
        self.scan_all("instructions by string prefix", |dex_idx, dex| {
            let matches = dex.strings().with_prefix(prefix).into_iter().collect();
            string_loads(dex_idx, dex, &matches)
        })
    }
}

fn string_loads(
    dex_idx: usize,
    dex: &DexFile,
    strings: &HashSet<StringIdx>,
) -> DexResult<Vec<InstructionLocation>> {
    if strings.is_empty() {
        return Ok(Vec::new());
    }
    let query = RefQuery::any_of(strings.iter().map(|&s| RefKey::String(s)));
    dex.scan_instructions(&query, |site| {
        site.instruction
            .string_ref()
            .is_some_and(|string| strings.contains(&string))
            .then(|| instruction_location(dex_idx, site))
    })
}
