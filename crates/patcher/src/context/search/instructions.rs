// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use super::super::{InstructionLocation, PatchContext};
use super::{DexResult, FieldRefQuery, MethodRefQuery, instruction_location, resolve};
use crate::error::Result;
use reseam_apk::reseam_dex::{
    DexFile, FieldIdx, MethodIdx, ProtoIdx, RefKey, RefQuery, StringIdx, TypeIdx,
};

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

    /// Match referenced method IDs before scanning their indexed call sites. Platform
    /// references are included even when their declaring class is not in the APK.
    pub fn find_calls_matching(
        &self,
        query: &MethodRefQuery<'_>,
    ) -> Result<Vec<InstructionLocation>> {
        let MethodRefQuery {
            owner,
            name,
            return_type,
            parameters,
            required_parameters,
            parameter_count,
        } = *query;
        self.scan_all("calls matching reference", |dex_idx, dex| {
            let (Ok(owner), Ok(name)) = (
                resolve(owner, |v| dex.find_type_idx(v)),
                resolve(name, |v| dex.strings().find(v)),
            ) else {
                return Ok(Vec::new());
            };
            let matches_proto = |proto: ProtoIdx| {
                let proto = dex.proto(proto);
                let parameter = |t: &TypeIdx| dex.type_descriptor(*t);
                return_type.is_none_or(|v| dex.type_descriptor(proto.return_type) == v)
                    && parameters.is_none_or(|v| {
                        v.len() == proto.parameters.len()
                            && v.iter()
                                .zip(&proto.parameters)
                                .all(|(v, t)| *v == parameter(t))
                    })
                    && parameter_count.is_none_or(|count| count == proto.parameters.len())
                    && required_parameters
                        .iter()
                        .all(|v| proto.parameters.iter().any(|t| *v == parameter(t)))
            };
            // Owner/name typically select one or two IDs. Only precompute all
            // matching prototypes for a signature-only query.
            let matching_protos = if owner.is_none() && name.is_none() && query.constrains_proto() {
                Some(
                    (0..dex.prototypes().len())
                        .filter_map(|i| matches_proto(ProtoIdx(i as u32)).then_some(i as u32))
                        .collect::<HashSet<_>>(),
                )
            } else {
                None
            };
            let accepts_proto = |proto: ProtoIdx| {
                matching_protos
                    .as_ref()
                    .map_or_else(|| matches_proto(proto), |protos| protos.contains(&proto.0))
            };
            let members: HashSet<_> = match owner {
                Some(owner) => dex
                    .methods_of(owner, name)
                    .filter(|m| accepts_proto(dex.methods().get(m.0 as usize).proto))
                    .collect(),
                None => dex
                    .methods()
                    .iter()
                    .enumerate()
                    .filter_map(|(i, id)| {
                        (name.is_none_or(|v| id.name == v) && accepts_proto(id.proto))
                            .then_some(MethodIdx(i as u32))
                    })
                    .collect(),
            };
            if members.is_empty() {
                return Ok(Vec::new());
            }
            let query = RefQuery::any_of(members.iter().copied().map(RefKey::Method));
            dex.scan_instructions(&query, |site| {
                members
                    .contains(&site.instruction.method_ref()?)
                    .then(|| instruction_location(dex_idx, site))
            })
        })
    }

    pub fn find_field_accesses_matching(
        &self,
        query: &FieldRefQuery<'_>,
    ) -> Result<Vec<InstructionLocation>> {
        self.scan_all("field accesses matching reference", |dex_idx, dex| {
            let (Ok(owner), Ok(name), Ok(field_type)) = (
                resolve(query.owner, |v| dex.find_type_idx(v)),
                resolve(query.name, |v| dex.strings().find(v)),
                resolve(query.field_type, |v| dex.find_type_idx(v)),
            ) else {
                return Ok(Vec::new());
            };
            let accepts = |field: &FieldIdx| {
                let id = dex.fields().get(field.0 as usize);
                name.is_none_or(|v| id.name == v) && field_type.is_none_or(|v| id.type_ == v)
            };
            let members: HashSet<FieldIdx> = match owner {
                Some(owner) => dex.fields_of(owner, name).filter(accepts).collect(),
                None => (0..dex.fields().len() as u32)
                    .map(FieldIdx)
                    .filter(accepts)
                    .collect(),
            };
            if members.is_empty() {
                return Ok(Vec::new());
            }
            let query = RefQuery::any_of(members.iter().copied().map(RefKey::Field));
            dex.scan_instructions(&query, |site| {
                members
                    .contains(&site.instruction.field_ref()?)
                    .then(|| instruction_location(dex_idx, site))
            })
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
