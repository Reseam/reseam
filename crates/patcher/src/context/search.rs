// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{
    ClassLocation, FingerprintLocation, InstructionLocation, MethodLocation, PatchContext, SiteHit,
};
use crate::error::Result;
use reseam_apk::reseam_dex::{
    DexFile, Fingerprint, FingerprintHit, InstructionPattern, InstructionSite, MethodHit,
    MethodIdx, RefKey, RefQuery, StringIdx, TypeIdx,
};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use tracing::debug;

type DexResult<T> = reseam_apk::reseam_dex::Result<T>;

impl PatchContext<'_> {
    /// Every method whose prototype satisfies each given filter (exact return
    /// type, exact parameter list, a parameter of `contains` type anywhere),
    /// resolved through the id tables alone: no class data or code is decoded.
    /// A DEX missing any of the named types has no matches.
    pub fn find_methods_by_proto(
        &self,
        return_type: Option<&str>,
        parameters: Option<&[&str]>,
        contains: Option<&str>,
    ) -> Result<Vec<MethodLocation>> {
        self.scan_all("methods by prototype", |dex_idx, dex| {
            let resolve = |t: &str| dex.find_type_idx(t).ok_or(());
            let resolved = (|| {
                Ok::<_, ()>((
                    return_type.map(resolve).transpose()?,
                    parameters
                        .map(|types| {
                            types
                                .iter()
                                .map(|t| resolve(t))
                                .collect::<std::result::Result<Vec<_>, ()>>()
                        })
                        .transpose()?,
                    contains.map(resolve).transpose()?,
                ))
            })();
            let Ok((return_type, parameters, contains)) = resolved else {
                return Ok(Vec::new());
            };
            let hits = dex.scan_methods_collect(&RefQuery::default(), |view| {
                let proto = dex.proto(dex.method_id(view.method).proto);
                let matches = return_type.is_none_or(|t| proto.return_type == t)
                    && parameters
                        .as_deref()
                        .is_none_or(|types| proto.parameters.as_slice() == types)
                    && contains.is_none_or(|t| proto.parameters.contains(&t));
                Ok(matches.then(|| view.hit()))
            })?;
            Ok(hits
                .iter()
                .map(|hit| method_location(dex_idx, hit))
                .collect())
        })
    }

    pub fn find_methods_by_name(&self, method_name: &str) -> Result<Vec<MethodLocation>> {
        self.scan_all("methods by name", |dex_idx, dex| {
            let Some(name) = dex.find_string_idx(method_name) else {
                return Ok(Vec::new());
            };
            let hits = dex.scan_methods_collect(&RefQuery::default(), |view| {
                Ok((dex.method_id(view.method).name == name).then(|| view.hit()))
            })?;
            Ok(hits
                .iter()
                .map(|hit| method_location(dex_idx, hit))
                .collect())
        })
    }

    pub fn find_method_by_name(&self, method_name: &str) -> Result<Option<MethodLocation>> {
        self.scan_first("method by name", |dex_idx, dex| {
            Ok(dex
                .find_method_by_name(method_name)?
                .map(|hit| method_location(dex_idx, &hit)))
        })
    }

    pub fn find_methods_by_strings(&self, strings: &[&str]) -> Result<Vec<MethodLocation>> {
        self.scan_all("methods by strings", |dex_idx, dex| {
            let hits = dex.find_methods_by_strings(strings)?;
            Ok(hits
                .iter()
                .map(|hit| method_location(dex_idx, hit))
                .collect())
        })
    }

    pub fn find_methods_with_opcodes(
        &self,
        opcodes: &[InstructionPattern],
    ) -> Result<Vec<MethodLocation>> {
        self.scan_all("methods by opcodes", |dex_idx, dex| {
            let hits = dex.find_methods_with_opcodes(opcodes)?;
            Ok(hits
                .iter()
                .map(|hit| method_location(dex_idx, hit))
                .collect())
        })
    }

    pub fn find_method_by_fingerprint(
        &self,
        fp: &Fingerprint,
    ) -> Result<Option<FingerprintLocation>> {
        self.scan_first("method by fingerprint", |dex_idx, dex| {
            Ok(dex
                .find_method_by_fingerprint(fp)?
                .map(|hit| fingerprint_location(dex_idx, hit)))
        })
    }

    pub fn find_methods_by_fingerprint(
        &self,
        fp: &Fingerprint,
    ) -> Result<Vec<FingerprintLocation>> {
        self.scan_all("methods by fingerprint", |dex_idx, dex| {
            let hits = dex.find_methods_by_fingerprint(fp)?;
            Ok(hits
                .into_iter()
                .map(|hit| fingerprint_location(dex_idx, hit))
                .collect())
        })
    }

    /// Visits every method in the APK in DEX, class, then member order
    /// without collecting them first.
    pub fn for_each_method(&self, mut visit: impl FnMut(MethodLocation)) -> Result<()> {
        for (dex_idx, dex) in self.dex().iter().enumerate() {
            dex.scan_methods_find(&RefQuery::default(), |view| {
                visit(method_location(dex_idx, &view.hit()));
                Ok(None::<()>)
            })?;
        }
        Ok(())
    }

    pub fn find_instructions_by_literal(&self, literal: i64) -> Result<Vec<InstructionLocation>> {
        self.scan_all("instructions by literal", |dex_idx, dex| {
            dex.scan_instructions(&RefQuery::all_of([RefKey::literal(literal)]), |site| {
                (site.instruction.literal() == Some(literal))
                    .then(|| instruction_location(dex_idx, site))
            })
        })
    }

    pub fn find_instructions_by_string(&self, target: &str) -> Result<Vec<InstructionLocation>> {
        self.scan_all("instructions by string", |dex_idx, dex| {
            let Some(target_idx) = dex.find_string_idx(target) else {
                return Ok(Vec::new());
            };
            dex.scan_instructions(&RefQuery::all_of([RefKey::string(target_idx)]), |site| {
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
            let matches: HashSet<StringIdx> = dex
                .strings()
                .iter()
                .enumerate()
                .filter(|(_, s)| s.contains(substring))
                .map(|(i, _)| StringIdx(i as u32))
                .collect();
            if matches.is_empty() {
                return Ok(Vec::new());
            }
            let query = RefQuery::any_of(matches.iter().map(|&s| RefKey::string(s)));
            dex.scan_instructions(&query, |site| {
                site.instruction
                    .string_ref()
                    .is_some_and(|sref| matches.contains(&sref))
                    .then(|| instruction_location(dex_idx, site))
            })
        })
    }

    /// Call sites of `(class, method)` targets; hits carry the target's index.
    pub fn find_method_call_sites(&self, targets: &[(String, String)]) -> Result<Vec<SiteHit>> {
        self.scan_all("method call sites", |dex_idx, dex| {
            let map = member_targets(dex, targets, |class, name| {
                dex.methods_of(class, Some(name))
            });
            if map.is_empty() {
                return Ok(Vec::new());
            }
            let query = RefQuery::any_of(map.keys().map(|&m| RefKey::method(m)));
            dex.scan_instructions(&query, |site| {
                let target_index = *map.get(&site.instruction.method_ref()?)?;
                Some(SiteHit {
                    loc: instruction_location(dex_idx, site),
                    target_index,
                })
            })
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
            let owner = match owner {
                Some(v) => match dex.find_type_idx(v) {
                    Some(v) => Some(v),
                    None => return Ok(Vec::new()),
                },
                None => None,
            };
            let name = match name {
                Some(v) => match dex.find_string_idx(v) {
                    Some(v) => Some(v),
                    None => return Ok(Vec::new()),
                },
                None => None,
            };
            let matches_proto = |proto: reseam_apk::reseam_dex::ProtoIdx| {
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
                        .filter_map(|i| {
                            matches_proto(reseam_apk::reseam_dex::ProtoIdx(i as u32))
                                .then_some(i as u32)
                        })
                        .collect::<HashSet<_>>(),
                )
            } else {
                None
            };
            let accepts_proto = |proto: reseam_apk::reseam_dex::ProtoIdx| {
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
            let query = RefQuery::any_of(members.iter().copied().map(RefKey::method));
            dex.scan_instructions(&query, |site| {
                members
                    .contains(&site.instruction.method_ref()?)
                    .then(|| instruction_location(dex_idx, site))
            })
        })
    }

    pub fn find_classes_with_instance_field(&self, field_type: &str) -> Result<Vec<ClassLocation>> {
        self.scan_all("classes with instance field", |dex_idx, dex| {
            let Some(wanted) = dex.find_type_idx(field_type) else {
                return Ok(Vec::new());
            };
            let mut matches = Vec::new();
            for class_idx in 0..dex.classes().len() {
                if dex
                    .decode_class_fields(class_idx)?
                    .is_some_and(|(_, fields)| {
                        fields.iter().any(|f| dex.field_id(f.field).type_ == wanted)
                    })
                {
                    matches.push(ClassLocation { dex_idx, class_idx });
                }
            }
            Ok(matches)
        })
    }

    /// Accesses of `(class, field)` targets; hits carry the target's index.
    pub fn find_field_access_sites(&self, targets: &[(String, String)]) -> Result<Vec<SiteHit>> {
        self.scan_all("field access sites", |dex_idx, dex| {
            let map = member_targets(dex, targets, |class, name| dex.fields_of(class, Some(name)));
            if map.is_empty() {
                return Ok(Vec::new());
            }
            let query = RefQuery::any_of(map.keys().map(|&f| RefKey::field(f)));
            dex.scan_instructions(&query, |site| {
                let target_index = *map.get(&site.instruction.field_ref()?)?;
                Some(SiteHit {
                    loc: instruction_location(dex_idx, site),
                    target_index,
                })
            })
        })
    }

    fn scan_all<T>(
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

    pub(super) fn scan_first<T>(
        &self,
        what: &str,
        scan: impl Fn(usize, &DexFile) -> DexResult<Option<T>>,
    ) -> Result<Option<T>> {
        debug!(what, "scan");
        for (index, dex) in self.dex().iter().enumerate() {
            if let Some(hit) = scan(index, dex)? {
                return Ok(Some(hit));
            }
        }
        Ok(None)
    }
}

/// A method reference to find calls to. Unset parts match anything;
/// `required_parameters` must each appear somewhere in the parameter list.
#[derive(Debug, Clone, Copy, Default)]
pub struct MethodRefQuery<'a> {
    pub owner: Option<&'a str>,
    pub name: Option<&'a str>,
    pub return_type: Option<&'a str>,
    pub parameters: Option<&'a [&'a str]>,
    pub required_parameters: &'a [&'a str],
    pub parameter_count: Option<usize>,
}

impl MethodRefQuery<'_> {
    fn constrains_proto(&self) -> bool {
        self.return_type.is_some()
            || self.parameters.is_some()
            || !self.required_parameters.is_empty()
            || self.parameter_count.is_some()
    }
}

fn member_targets<K: Hash + Eq, I: Iterator<Item = K>>(
    dex: &DexFile,
    targets: &[(String, String)],
    members: impl Fn(TypeIdx, StringIdx) -> I,
) -> HashMap<K, usize> {
    targets
        .iter()
        .enumerate()
        .filter_map(|(index, (class, name))| {
            Some((index, dex.find_type_idx(class)?, dex.find_string_idx(name)?))
        })
        .flat_map(|(index, class, name)| members(class, name).map(move |id| (id, index)))
        .fold(HashMap::new(), |mut map, (id, index)| {
            map.entry(id).or_insert(index);
            map
        })
}

fn method_location(dex_idx: usize, hit: &MethodHit) -> MethodLocation {
    MethodLocation {
        dex_idx,
        class_idx: hit.class_idx,
        method_idx: hit.method_pos,
        kind: hit.kind,
    }
}

fn fingerprint_location(dex_idx: usize, hit: FingerprintHit) -> FingerprintLocation {
    FingerprintLocation {
        method: method_location(dex_idx, &hit.method),
        matched_indices: hit.matched_indices,
    }
}

fn instruction_location(dex_idx: usize, site: &InstructionSite<'_>) -> InstructionLocation {
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
