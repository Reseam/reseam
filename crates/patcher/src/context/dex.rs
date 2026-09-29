// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Searching the APK's DEX files. Every finder runs one scan per DEX through
//! [`PatchContext::scan_all`] or [`PatchContext::scan_first`]; a DEX whose
//! scan fails is logged and skipped.

use std::collections::HashMap;
use std::hash::Hash;

use reseam_apk::reseam_dex::{
    summarize_resident, DexFile, EncodedField, EncodedMethod, Fingerprint, FingerprintHit,
    InstructionPattern, InstructionSite, MemberCounts, MethodHit, MethodIdx, MethodSummary,
    MultiDexContainer, RefKey, RefQuery, StringIdx, TypeIdx,
};
use tracing::{debug, warn};

use super::{
    CachedMethod, CachedSkeleton, ClassLocation, FingerprintLocation, InstructionLocation,
    MethodLocation, PatchContext, SiteHit,
};

type DexResult<T> = reseam_apk::reseam_dex::Result<T>;

impl<'a> PatchContext<'a> {
    pub fn dex(&self) -> &MultiDexContainer {
        self.apk.dex()
    }

    pub fn dex_file(&self, index: usize) -> Option<&DexFile> {
        self.apk.dex().dex(index)
    }

    /// One DEX for whole-file operations such as interning, without
    /// resolving any class data.
    pub fn dex_file_mut(&mut self, index: usize) -> Option<&mut DexFile> {
        self.method = None;
        self.apk.dex_mut(index)
    }

    /// Reads one method for inspection without materializing its class. The
    /// decode is cached until the next mutable DEX access, so read-only FFIs
    /// that walk a method instruction by instruction decode it once.
    pub fn read_method(&mut self, location: MethodLocation) -> Option<(&DexFile, &EncodedMethod)> {
        if self
            .method
            .as_ref()
            .is_none_or(|cached| cached.location != location)
        {
            let method = self
                .dex_file(location.dex_idx)?
                .decode_method_at(location.class_idx, location.method_idx, location.is_virtual)
                .ok()
                .flatten()?;
            self.method = Some(CachedMethod { location, method });
        }
        Some((
            self.apk.dex().dex(location.dex_idx)?,
            &self.method.as_ref()?.method,
        ))
    }

    /// A method's identity and frame shape without decoding its code. Deferred
    /// classes go through the cached skeleton so walking a class's methods
    /// costs one raw read per class instead of one per method.
    pub fn read_method_summary(&mut self, location: MethodLocation) -> Option<MethodSummary> {
        let MethodLocation {
            dex_idx,
            class_idx,
            method_idx: method_pos,
            is_virtual,
        } = location;
        let dex = self.apk.dex().dex(dex_idx)?;
        if let Some(data) = dex
            .resident_class(class_idx)
            .and_then(|c| c.class_data.as_deref())
        {
            let list = if is_virtual {
                &data.virtual_methods
            } else {
                &data.direct_methods
            };
            return list.get(method_pos).map(summarize_resident);
        }
        let location = ClassLocation { dex_idx, class_idx };
        if self
            .skeleton
            .as_ref()
            .is_none_or(|c| c.location != location)
        {
            let skeleton = dex.class_skeleton(class_idx).ok().flatten()?;
            self.skeleton = Some(CachedSkeleton { location, skeleton });
        }
        let header = self
            .skeleton
            .as_ref()?
            .skeleton
            .method(method_pos, is_virtual)?;
        dex.summarize_method(header).ok()
    }

    /// Class member counts without materializing the class.
    pub fn read_class_counts(&self, location: ClassLocation) -> Option<MemberCounts> {
        self.dex_file(location.dex_idx)?
            .class_member_counts(location.class_idx)
            .ok()
            .flatten()
    }

    /// Class fields `(static, instance)` for inspection without materializing
    /// the class. Returns the non-resolving DEX alongside the owned fields.
    pub fn read_class_fields(
        &self,
        location: ClassLocation,
    ) -> Option<(&DexFile, Vec<EncodedField>, Vec<EncodedField>)> {
        let dex = self.dex_file(location.dex_idx)?;
        let (statics, instances) = dex.decode_class_fields(location.class_idx).ok().flatten()?;
        Some((dex, statics, instances))
    }

    /// Materializes one located class and returns its DEX for mutation.
    pub fn class_dex_mut(&mut self, dex_idx: usize, class_idx: usize) -> Option<&mut DexFile> {
        self.method = None;
        self.apk
            .resolve_dex_class_mut(dex_idx, class_idx)
            .ok()
            .flatten()
    }

    pub fn find_class(&self, descriptor: &str) -> Option<ClassLocation> {
        self.scan_first("class", |dex_idx, dex| {
            Ok(dex
                .find_class_index(descriptor)
                .map(|class_idx| ClassLocation { dex_idx, class_idx }))
        })
    }

    /// Superclasses nearest first. A base class often sits in another DEX of a
    /// multi-dex app, so the walk resolves each superclass by descriptor across
    /// the whole app and stops where the app stops defining them.
    pub fn superclass_chain(&self, class: ClassLocation) -> Vec<ClassLocation> {
        let mut chain: Vec<ClassLocation> = Vec::new();
        let mut current = class;
        while let Some(superclass) = self.dex_file(current.dex_idx).and_then(|dex| {
            let header = dex.class_header(current.class_idx);
            header
                .superclass
                .map(|ty| dex.type_descriptor(ty).into_owned())
        }) {
            let Some(next) = self.find_class(&superclass) else {
                break;
            };
            if next == class || chain.contains(&next) {
                break;
            }
            chain.push(next);
            current = next;
        }
        chain
    }

    /// Locates a method by class and name without materializing the class:
    /// resident classes are searched through their IR, others through the
    /// raw member list.
    pub fn find_method(&self, class_descriptor: &str, method_name: &str) -> Option<MethodLocation> {
        self.scan_first("method", |dex_idx, dex| {
            let (Some(class_idx), Some(name)) = (
                dex.find_class_index(class_descriptor),
                dex.find_string_idx(method_name),
            ) else {
                return Ok(None);
            };
            let named = |method: MethodIdx| dex.method_id(method).name == name;
            let slot = match dex.resident_class(class_idx) {
                Some(class) => class.class_data.as_ref().and_then(|data| {
                    find_slot(
                        data.direct_methods.iter().map(|m| m.method),
                        data.virtual_methods.iter().map(|m| m.method),
                        named,
                    )
                }),
                None => dex.class_skeleton(class_idx)?.and_then(|skeleton| {
                    find_slot(
                        skeleton.direct_methods.iter().map(|m| m.method),
                        skeleton.virtual_methods.iter().map(|m| m.method),
                        named,
                    )
                }),
            };
            Ok(slot.map(|(method_idx, is_virtual)| MethodLocation {
                dex_idx,
                class_idx,
                method_idx,
                is_virtual,
            }))
        })
    }

    /// Every method whose prototype satisfies each given filter (exact return
    /// type, exact parameter list, a parameter of `contains` type anywhere),
    /// resolved through the id tables alone: no class data or code is decoded.
    /// A DEX missing any of the named types has no matches.
    pub fn find_methods_by_proto(
        &self,
        return_type: Option<&str>,
        parameters: Option<&[&str]>,
        contains: Option<&str>,
    ) -> Vec<MethodLocation> {
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
                                .collect::<Result<Vec<_>, ()>>()
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

    pub fn find_methods_by_name(&self, method_name: &str) -> Vec<MethodLocation> {
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

    pub fn find_method_by_name(&self, method_name: &str) -> Option<MethodLocation> {
        self.scan_first("method by name", |dex_idx, dex| {
            Ok(dex
                .find_method_by_name(method_name)?
                .map(|hit| method_location(dex_idx, &hit)))
        })
    }

    pub fn find_methods_by_strings(&self, strings: &[&str]) -> Vec<MethodLocation> {
        self.scan_all("methods by strings", |dex_idx, dex| {
            let hits = dex.find_methods_by_strings(strings)?;
            Ok(hits
                .iter()
                .map(|hit| method_location(dex_idx, hit))
                .collect())
        })
    }

    pub fn find_methods_with_opcodes(&self, opcodes: &[InstructionPattern]) -> Vec<MethodLocation> {
        self.scan_all("methods by opcodes", |dex_idx, dex| {
            let hits = dex.find_methods_with_opcodes(opcodes)?;
            Ok(hits
                .iter()
                .map(|hit| method_location(dex_idx, hit))
                .collect())
        })
    }

    pub fn find_method_by_fingerprint(&self, fp: &Fingerprint) -> Option<FingerprintLocation> {
        self.scan_first("method by fingerprint", |dex_idx, dex| {
            Ok(dex
                .find_method_by_fingerprint(fp)?
                .map(|hit| fingerprint_location(dex_idx, hit)))
        })
    }

    pub fn find_methods_by_fingerprint(&self, fp: &Fingerprint) -> Vec<FingerprintLocation> {
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
    pub fn for_each_method(&self, mut visit: impl FnMut(MethodLocation)) {
        for (dex_idx, dex) in self.dex().iter().enumerate() {
            let walked = dex.scan_methods_find(&RefQuery::default(), |view| {
                visit(method_location(dex_idx, &view.hit()));
                Ok(None::<()>)
            });
            ok_or_warn(dex_idx, "every method", walked);
        }
    }

    pub fn find_instructions_by_literal(&self, literal: i64) -> Vec<InstructionLocation> {
        self.scan_all("instructions by literal", |dex_idx, dex| {
            dex.scan_instructions(&RefQuery::all_of([RefKey::literal(literal)]), |site| {
                (site.instruction.literal() == Some(literal))
                    .then(|| instruction_location(dex_idx, site))
            })
        })
    }

    pub fn find_instructions_by_string(&self, target: &str) -> Vec<InstructionLocation> {
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
    ) -> Vec<InstructionLocation> {
        self.scan_all("instructions by substring", |dex_idx, dex| {
            let matches: Vec<StringIdx> = dex
                .strings
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
    pub fn find_method_call_sites(&self, targets: &[(String, String)]) -> Vec<SiteHit> {
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
    pub fn find_calls_matching(&self, query: &MethodRefQuery<'_>) -> Vec<InstructionLocation> {
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
                let parameter = |t: &reseam_apk::reseam_dex::TypeIdx| dex.type_descriptor(*t);
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
                    (0..dex.prototypes.len())
                        .filter_map(|i| {
                            matches_proto(reseam_apk::reseam_dex::ProtoIdx(i as u16))
                                .then_some(i as u16)
                        })
                        .collect::<std::collections::HashSet<_>>(),
                )
            } else {
                None
            };
            let accepts_proto = |proto: reseam_apk::reseam_dex::ProtoIdx| {
                matching_protos
                    .as_ref()
                    .map_or_else(|| matches_proto(proto), |protos| protos.contains(&proto.0))
            };
            let members: std::collections::HashSet<_> = match owner {
                Some(owner) => dex
                    .methods_of(owner, name)
                    .filter(|m| accepts_proto(dex.methods.get(m.0 as usize).proto))
                    .collect(),
                None => dex
                    .methods
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

    /// Read encoded fields in one native pass, without decoding method bodies.
    pub fn find_classes_with_instance_field(&self, field_type: &str) -> Vec<ClassLocation> {
        self.scan_all("classes with instance field", |dex_idx, dex| {
            let Some(wanted) = dex.find_type_idx(field_type) else {
                return Ok(Vec::new());
            };
            let mut matches = Vec::new();
            for class_idx in 0..dex.classes.len() {
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
    pub fn find_field_access_sites(&self, targets: &[(String, String)]) -> Vec<SiteHit> {
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
    ) -> Vec<T> {
        debug!(what, "full scan");
        self.dex()
            .iter()
            .enumerate()
            .flat_map(|(dex_idx, dex)| ok_or_warn(dex_idx, what, scan(dex_idx, dex)))
            .collect()
    }

    fn scan_first<T>(
        &self,
        what: &str,
        scan: impl Fn(usize, &DexFile) -> DexResult<Option<T>>,
    ) -> Option<T> {
        debug!(what, "scan");
        self.dex()
            .iter()
            .enumerate()
            .find_map(|(dex_idx, dex)| ok_or_warn(dex_idx, what, scan(dex_idx, dex)))
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

fn find_slot(
    mut direct: impl Iterator<Item = MethodIdx>,
    mut virtual_: impl Iterator<Item = MethodIdx>,
    named: impl Fn(MethodIdx) -> bool,
) -> Option<(usize, bool)> {
    direct
        .position(&named)
        .map(|pos| (pos, false))
        .or_else(|| virtual_.position(named).map(|pos| (pos, true)))
}

/// Maps each member id whose class and name match one of `targets` to that
/// target's index in `targets`.
/// Maps each member a target names to that target's index. `members` lists
/// the members declared on a class under a name.
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

fn ok_or_warn<T: Default>(dex_idx: usize, what: &str, result: DexResult<T>) -> T {
    result.unwrap_or_else(|error| {
        warn!(dex_idx, %error, what, "dex scan failed");
        T::default()
    })
}

fn method_location(dex_idx: usize, hit: &MethodHit) -> MethodLocation {
    MethodLocation {
        dex_idx,
        class_idx: hit.class_idx,
        method_idx: hit.method_pos,
        is_virtual: hit.is_virtual,
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
            is_virtual: site.is_virtual,
        },
        insn_idx: site.insn_idx,
    }
}
