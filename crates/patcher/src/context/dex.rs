// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::path::Path;

use reseam_apk::reseam_dex::{summarize_resident, MethodSummary, 
    find_free_register, find_free_registers, ClassDef, CodeItem, DexFile, EncodedField,
    EncodedMethod, Fingerprint, FingerprintHit, InstructionPattern, InstructionSite, MemberCounts,
    MethodHit, MethodIdx, MultiDexContainer, ParseOptions, StringIdx,
};
use tracing::{debug, warn};

use super::{
    CachedMethod, CachedSkeleton, ClassLocation, FieldAccessSiteHit, FingerprintLocation,
    InstructionLocation, MethodCallSiteHit, MethodLocation, PatchContext,
};
use reseam_apk::reseam_dex::{Prototype, RefKey, RefQuery, TypeIdx};
use crate::error::{PatcherError, Result as PatcherResult};

type DexResult<T> = reseam_apk::reseam_dex::Result<T>;

impl<'a> PatchContext<'a> {
    pub fn dex(&self) -> &MultiDexContainer {
        self.apk.dex()
    }

    pub fn dex_count(&self) -> usize {
        self.apk.dex().len()
    }

    pub fn dex_file(&self, index: usize) -> Option<&DexFile> {
        self.apk.dex().dex(index)
    }

    pub fn dex_file_mut(&mut self, index: usize) -> Option<&mut DexFile> {
        self.method = None;
        self.apk.dex_mut_at(index)
    }

    pub fn dex_mut(&mut self, index: usize) -> PatcherResult<&mut DexFile> {
        self.dex_file_mut(index)
            .ok_or_else(|| PatcherError::NotFound(format!("dex index {index}")))
    }

    /// Materializes one located class and returns its DEX immutably.
    pub fn class_dex(&mut self, dex_idx: usize, class_idx: usize) -> Option<&DexFile> {
        self.apk.resolve_dex_class(dex_idx, class_idx).ok().flatten()
    }

    /// Reads one method for inspection without materializing its class. The
    /// decode is cached until the next mutable DEX access, so read-only FFIs
    /// that walk a method instruction by instruction decode it once.
    pub fn read_method(
        &mut self,
        dex_idx: usize,
        class_idx: usize,
        method_pos: usize,
        is_virtual: bool,
    ) -> Option<(&DexFile, &EncodedMethod)> {
        let location = MethodLocation {
            dex_idx,
            class_idx,
            method_idx: method_pos,
            is_virtual,
        };
        if self.method.as_ref().is_none_or(|cached| cached.location != location) {
            let method = self
                .dex_file(dex_idx)?
                .decode_method_at(class_idx, method_pos, is_virtual)
                .ok()
                .flatten()?;
            self.method = Some(CachedMethod { location, method });
        }
        Some((self.apk.dex().dex(dex_idx)?, &self.method.as_ref()?.method))
    }

    /// A method's identity and frame shape without decoding its code. Deferred
    /// classes go through the cached skeleton so walking a class's methods
    /// costs one raw read per class instead of one per method.
    pub fn read_method_summary(
        &mut self,
        dex_idx: usize,
        class_idx: usize,
        method_pos: usize,
        is_virtual: bool,
    ) -> Option<MethodSummary> {
        let dex = self.apk.dex().dex(dex_idx)?;
        if let Some(data) = dex.resident_class(class_idx).and_then(|c| c.class_data.as_deref()) {
            let list = if is_virtual {
                &data.virtual_methods
            } else {
                &data.direct_methods
            };
            return list.get(method_pos).map(summarize_resident);
        }
        let cached = self
            .skeleton
            .as_ref()
            .is_some_and(|c| c.dex_idx == dex_idx && c.class_idx == class_idx);
        if !cached {
            let skeleton = dex.class_skeleton(class_idx).ok().flatten()?;
            self.skeleton = Some(CachedSkeleton {
                dex_idx,
                class_idx,
                skeleton,
            });
        }
        let header = self.skeleton.as_ref()?.skeleton.method(method_pos, is_virtual)?;
        dex.summarize_method(header).ok()
    }

    /// Class member counts without materializing the class.
    pub fn read_class_counts(&self, dex_idx: usize, class_idx: usize) -> Option<MemberCounts> {
        self.dex_file(dex_idx)?
            .class_member_counts(class_idx)
            .ok()
            .flatten()
    }

    /// Class fields `(static, instance)` for inspection without materializing
    /// the class. Returns the non-resolving DEX alongside the owned fields.
    pub fn read_class_fields(
        &self,
        dex_idx: usize,
        class_idx: usize,
    ) -> Option<(&DexFile, Vec<EncodedField>, Vec<EncodedField>)> {
        let dex = self.dex_file(dex_idx)?;
        let (statics, instances) = dex.decode_class_fields(class_idx).ok().flatten()?;
        Some((dex, statics, instances))
    }

    /// Materializes one located class, marks its DEX modified, returns it mutably.
    pub fn class_dex_mut(&mut self, dex_idx: usize, class_idx: usize) -> Option<&mut DexFile> {
        self.method = None;
        self.apk
            .resolve_dex_class_mut(dex_idx, class_idx)
            .ok()
            .flatten()
    }

    pub fn find_class(&self, descriptor: &str) -> Option<ClassLocation> {
        for dex_idx in 0..self.dex_count() {
            let class_idx = self.apk.dex().dex(dex_idx)?.find_class_index(descriptor);
            if let Some(class_idx) = class_idx {
                return Some(ClassLocation { dex_idx, class_idx });
            }
        }
        None
    }

    pub fn find_class_mut(&mut self, descriptor: &str) -> Option<(ClassLocation, &mut ClassDef)> {
        let location = self.find_class(descriptor)?;
        let dex = self.class_dex_mut(location.dex_idx, location.class_idx)?;
        let class = dex.class_mut(location.class_idx).ok()?;
        Some((location, class))
    }

    pub fn class_mut(&mut self, descriptor: &str) -> PatcherResult<(ClassLocation, &mut ClassDef)> {
        self.find_class_mut(descriptor)
            .ok_or_else(|| PatcherError::NotFound(format!("class {descriptor}")))
    }

    /// Locates a method by class and name without materializing the class:
    /// resident classes are searched through their IR, others through the
    /// raw member list.
    pub fn find_method(&self, class_descriptor: &str, method_name: &str) -> Option<MethodLocation> {
        for dex_idx in 0..self.dex_count() {
            let dex = self.dex_file(dex_idx)?;
            let Some(class_idx) = dex.find_class_index(class_descriptor) else {
                continue;
            };
            let Some(name) = dex.find_string_idx(method_name) else {
                continue;
            };
            let named = |method: MethodIdx| dex.method_id(method).name == name;
            let slot = match dex.resident_class(class_idx) {
                Some(class) => {
                    let data = class.class_data.as_ref()?;
                    find_slot(data.direct_methods.iter().map(|m| m.method), data.virtual_methods.iter().map(|m| m.method), named)
                }
                None => {
                    let skeleton = dex.class_skeleton(class_idx).ok().flatten()?;
                    find_slot(skeleton.direct_methods.iter().map(|m| m.method), skeleton.virtual_methods.iter().map(|m| m.method), named)
                }
            };
            if let Some((method_idx, is_virtual)) = slot {
                return Some(MethodLocation {
                    dex_idx,
                    class_idx,
                    method_idx,
                    is_virtual,
                });
            }
        }
        None
    }

    pub fn find_method_mut(
        &mut self,
        class_descriptor: &str,
        method_name: &str,
    ) -> Option<(MethodLocation, &mut EncodedMethod)> {
        let location = self.find_method(class_descriptor, method_name)?;
        let dex = self.class_dex_mut(location.dex_idx, location.class_idx)?;
        let method = method_mut_at(dex, location)?;
        Some((location, method))
    }

    pub fn method_mut(
        &mut self,
        class_descriptor: &str,
        method_name: &str,
    ) -> PatcherResult<(MethodLocation, &mut EncodedMethod)> {
        self.find_method_mut(class_descriptor, method_name)
            .ok_or_else(|| PatcherError::NotFound(format!("{class_descriptor}.{method_name}")))
    }

    /// Every method whose prototype satisfies `matches`, resolved through the
    /// id tables alone: no class data or code is decoded. `types` are resolved
    /// per DEX; a DEX missing any of them is skipped.
    fn find_methods_by_proto(
        &self,
        types: &[&str],
        matches: impl Fn(&Prototype, &[TypeIdx]) -> bool + Sync,
    ) -> Vec<MethodLocation> {
        debug!(?types, "full scan: methods by prototype");
        let mut results = Vec::new();
        for dex_idx in 0..self.dex_count() {
            let Some(dex) = self.dex_file(dex_idx) else {
                continue;
            };
            let Some(resolved) = types
                .iter()
                .map(|t| dex.find_type_idx(t))
                .collect::<Option<Vec<_>>>()
            else {
                continue;
            };
            let hits = ok_or_warn(
                dex_idx,
                "find_methods_by_proto",
                dex.scan_methods_collect(&RefQuery::default(), |view| {
                    let proto = dex.proto(dex.method_id(view.method).proto);
                    Ok(matches(&proto, &resolved).then(|| view.hit()))
                }),
            );
            results.extend(hits.iter().map(|hit| method_location(dex_idx, hit)));
        }
        results
    }

    pub fn find_methods_by_return_type(&self, return_type: &str) -> Vec<MethodLocation> {
        self.find_methods_by_proto(&[return_type], |proto, types| proto.return_type == types[0])
    }

    pub fn find_methods_by_parameter_types(&self, parameter_types: &[&str]) -> Vec<MethodLocation> {
        self.find_methods_by_proto(parameter_types, |proto, types| proto.parameters.as_slice() == types)
    }

    pub fn find_methods_with_parameter(&self, parameter_type: &str) -> Vec<MethodLocation> {
        self.find_methods_by_proto(&[parameter_type], |proto, types| proto.parameters.contains(&types[0]))
    }

    pub fn find_method_by_name(&self, method_name: &str) -> Option<MethodLocation> {
        debug!(method_name, "full scan: method by name");
        for dex_idx in 0..self.dex_count() {
            let Some(dex) = self.dex_file(dex_idx) else {
                continue;
            };
            let hit = ok_or_warn(dex_idx, "find_method_by_name", dex.find_method_by_name(method_name));
            if let Some(hit) = hit {
                return Some(method_location(dex_idx, &hit));
            }
        }
        None
    }

    pub fn find_methods_by_strings(&self, strings: &[&str]) -> Vec<MethodLocation> {
        debug!(?strings, "full scan: methods by strings");
        let mut results = Vec::new();
        for dex_idx in 0..self.dex_count() {
            let Some(dex) = self.dex_file(dex_idx) else {
                continue;
            };
            let hits = ok_or_warn(
                dex_idx,
                "find_methods_by_strings",
                dex.find_methods_by_strings(strings),
            );
            results.extend(hits.iter().map(|hit| method_location(dex_idx, hit)));
        }
        results
    }

    pub fn find_methods_with_opcodes(
        &self,
        opcodes: &[InstructionPattern],
    ) -> Vec<MethodLocation> {
        debug!(patterns = opcodes.len(), "full scan: methods by opcodes");
        let mut results = Vec::new();
        for dex_idx in 0..self.dex_count() {
            let Some(dex) = self.dex_file(dex_idx) else {
                continue;
            };
            let hits = ok_or_warn(
                dex_idx,
                "find_methods_with_opcodes",
                dex.find_methods_with_opcodes(opcodes),
            );
            results.extend(hits.iter().map(|hit| method_location(dex_idx, hit)));
        }
        results
    }

    pub fn find_method_by_fingerprint(&self, fp: &Fingerprint) -> Option<FingerprintLocation> {
        debug!(name = ?fp.name, class = ?fp.defining_class, "full scan: method by fingerprint");
        for dex_idx in 0..self.dex_count() {
            let Some(dex) = self.dex_file(dex_idx) else {
                continue;
            };
            let hit = ok_or_warn(
                dex_idx,
                "find_method_by_fingerprint",
                dex.find_method_by_fingerprint(fp),
            );
            if let Some(hit) = hit {
                return Some(fingerprint_location(dex_idx, hit));
            }
        }
        None
    }

    /// Visits every method in the APK in DEX, class, then member order
    /// without collecting them first.
    pub fn for_each_method(&self, mut visit: impl FnMut(MethodLocation)) {
        for dex_idx in 0..self.dex_count() {
            let Some(dex) = self.dex_file(dex_idx) else {
                continue;
            };
            let walked = dex.scan_methods_find(&RefQuery::default(), |view| {
                visit(method_location(dex_idx, &view.hit()));
                Ok(None::<()>)
            });
            ok_or_warn(dex_idx, "for_each_method", walked);
        }
    }

    pub fn find_methods_by_fingerprint(&self, fp: &Fingerprint) -> Vec<FingerprintLocation> {
        debug!(name = ?fp.name, class = ?fp.defining_class, "full scan: methods by fingerprint");
        let mut results = Vec::new();
        for dex_idx in 0..self.dex_count() {
            let Some(dex) = self.dex_file(dex_idx) else {
                continue;
            };
            let hits = ok_or_warn(
                dex_idx,
                "find_methods_by_fingerprint",
                dex.find_methods_by_fingerprint(fp),
            );
            results.extend(hits.into_iter().map(|hit| fingerprint_location(dex_idx, hit)));
        }
        results
    }

    pub fn merge_extension_dex(&mut self, paths: &[impl AsRef<Path>]) -> PatcherResult<usize> {
        let mut count = 0;
        for path in paths {
            let path = path.as_ref();
            let bytes = std::fs::read(path).map_err(|e| PatcherError::Bundle {
                reason: format!("failed to read extension DEX {}: {e}", path.display()),
            })?;
            let dex = reseam_apk::reseam_dex::parse_owned(bytes, ParseOptions::default()).map_err(
                |e| PatcherError::Bundle {
                    reason: format!("failed to parse extension DEX {}: {e}", path.display()),
                },
            )?;
            self.apk.dex_mut().add_dex(dex);
            count += 1;
        }
        Ok(count)
    }

    pub fn find_free_register(
        &self,
        code: &CodeItem,
        at_index: usize,
        exclude: &[u16],
    ) -> Option<u16> {
        find_free_register(code, at_index, exclude)
    }

    pub fn find_free_registers(
        &self,
        code: &CodeItem,
        at_index: usize,
        count: usize,
        exclude: &[u16],
    ) -> Option<Vec<u16>> {
        find_free_registers(code, at_index, count, exclude)
    }

    pub fn find_instructions_by_literal(&self, literal: i64) -> Vec<InstructionLocation> {
        let query = RefQuery::all_of([RefKey::literal(literal)]);
        self.scan_instructions("find_instructions_by_literal", &query, |dex_idx, _dex, site| {
            (site.instruction.literal() == Some(literal)).then(|| instruction_location(dex_idx, site))
        })
    }

    pub fn find_instructions_by_string(&self, target: &str) -> Vec<InstructionLocation> {
        debug!(target, "full scan: instructions by string");
        let mut results = Vec::new();
        for dex_idx in 0..self.dex_count() {
            let Some(dex) = self.dex_file(dex_idx) else {
                continue;
            };
            let Some(target_idx) = dex.find_string_idx(target) else {
                continue;
            };
            let hits = ok_or_warn(
                dex_idx,
                "find_instructions_by_string",
                dex.scan_instructions(&RefQuery::all_of([RefKey::string(target_idx)]), |site| {
                    (site.instruction.string_ref() == Some(target_idx))
                        .then(|| instruction_location(dex_idx, site))
                }),
            );
            results.extend(hits);
        }
        results
    }

    pub fn find_instructions_by_string_contains(
        &self,
        substring: &str,
    ) -> Vec<InstructionLocation> {
        debug!(substring, "full scan: instructions by string contains");
        let mut results = Vec::new();
        for dex_idx in 0..self.dex_count() {
            let Some(dex) = self.dex_file(dex_idx) else {
                continue;
            };
            let matches: Vec<StringIdx> = dex
                .strings
                .iter()
                .enumerate()
                .filter(|(_, s)| s.contains(substring))
                .map(|(i, _)| StringIdx(i as u32))
                .collect();
            if matches.is_empty() {
                continue;
            }
            let query = RefQuery::any_of(matches.iter().map(|&s| RefKey::string(s)));
            let hits = ok_or_warn(
                dex_idx,
                "find_instructions_by_string_contains",
                dex.scan_instructions(&query, |site| {
                    site.instruction
                        .string_ref()
                        .is_some_and(|sref| matches.contains(&sref))
                        .then(|| instruction_location(dex_idx, site))
                }),
            );
            results.extend(hits);
        }
        results
    }

    pub fn find_method_call_sites(
        &self,
        targets: &[(String, String)],
    ) -> Vec<MethodCallSiteHit> {
        debug!(?targets, "full scan: method call sites");
        let mut results = Vec::new();
        for dex_idx in 0..self.dex_count() {
            let Some(dex) = self.dex_file(dex_idx) else {
                continue;
            };
            let mut target_map: HashMap<MethodIdx, usize> = HashMap::new();
            for (target_idx, (class_desc, method_name)) in targets.iter().enumerate() {
                let (Some(class), Some(name)) =
                    (dex.find_type_idx(class_desc), dex.find_string_idx(method_name))
                else {
                    continue;
                };
                for (i, mid) in dex.methods.iter().enumerate() {
                    if mid.class == class && mid.name == name {
                        target_map.insert(MethodIdx(i as u32), target_idx);
                    }
                }
            }
            if target_map.is_empty() {
                continue;
            }
            let query = RefQuery::any_of(target_map.keys().map(|&m| RefKey::method(m)));
            let hits = ok_or_warn(
                dex_idx,
                "find_method_call_sites",
                dex.scan_instructions(&query, |site| {
                    let target_index = *target_map.get(&site.instruction.method_ref()?)?;
                    Some(MethodCallSiteHit {
                        loc: instruction_location(dex_idx, site),
                        target_index,
                    })
                }),
            );
            results.extend(hits);
        }
        results
    }

    pub fn find_field_access_sites(
        &self,
        targets: &[(String, String)],
    ) -> Vec<FieldAccessSiteHit> {
        debug!(?targets, "full scan: field access sites");
        let mut results = Vec::new();
        for dex_idx in 0..self.dex_count() {
            let Some(dex) = self.dex_file(dex_idx) else {
                continue;
            };
            let mut target_map: HashMap<reseam_apk::reseam_dex::FieldIdx, usize> = HashMap::new();
            for (target_idx, (class_desc, field_name)) in targets.iter().enumerate() {
                let (Some(class), Some(name)) =
                    (dex.find_type_idx(class_desc), dex.find_string_idx(field_name))
                else {
                    continue;
                };
                for (i, fid) in dex.fields.iter().enumerate() {
                    if fid.class == class && fid.name == name {
                        target_map.insert(reseam_apk::reseam_dex::FieldIdx(i as u32), target_idx);
                    }
                }
            }
            if target_map.is_empty() {
                continue;
            }
            let query = RefQuery::any_of(target_map.keys().map(|&f| RefKey::field(f)));
            let hits = ok_or_warn(
                dex_idx,
                "find_field_access_sites",
                dex.scan_instructions(&query, |site| {
                    let target_index = *target_map.get(&site.instruction.field_ref()?)?;
                    Some(FieldAccessSiteHit {
                        loc: instruction_location(dex_idx, site),
                        target_index,
                    })
                }),
            );
            results.extend(hits);
        }
        results
    }

    fn scan_instructions<T: Send>(
        &self,
        what: &str,
        query: &RefQuery,
        matcher: impl Fn(usize, &DexFile, &InstructionSite<'_>) -> Option<T> + Sync,
    ) -> Vec<T> {
        debug!(what, "full scan: instructions");
        let mut results = Vec::new();
        for dex_idx in 0..self.dex_count() {
            let Some(dex) = self.dex_file(dex_idx) else {
                continue;
            };
            let hits = ok_or_warn(
                dex_idx,
                what,
                dex.scan_instructions(query, |site| matcher(dex_idx, dex, site)),
            );
            results.extend(hits);
        }
        results
    }

    pub fn resolve_fingerprint_location(
        &mut self,
        fp_match: &FingerprintLocation,
    ) -> PatcherResult<(String, String)> {
        self.resolve_method_location(fp_match.method)
    }

    pub fn resolve_method_location(
        &mut self,
        location: MethodLocation,
    ) -> PatcherResult<(String, String)> {
        self.resolve_names(location.dex_idx, location.class_idx, location.method_idx, location.is_virtual)
    }

    pub fn resolve_literal_location(
        &mut self,
        loc: &InstructionLocation,
    ) -> PatcherResult<(String, String)> {
        self.resolve_names(loc.dex_idx, loc.class_idx, loc.method_pos, loc.is_virtual)
    }

    /// `(class descriptor, method name)` of a located method, read without
    /// materializing its class.
    fn resolve_names(
        &self,
        dex_idx: usize,
        class_idx: usize,
        method_pos: usize,
        is_virtual: bool,
    ) -> PatcherResult<(String, String)> {
        let dex = self
            .dex_file(dex_idx)
            .ok_or_else(|| PatcherError::NotFound(format!("dex {dex_idx}")))?;
        if class_idx >= dex.classes.len() {
            return Err(PatcherError::NotFound(format!("class index {class_idx}")));
        }
        let class_desc = dex.type_descriptor(dex.class_header(class_idx).class_type).to_string();
        let summary = dex
            .method_summary(class_idx, method_pos, is_virtual)
            .ok()
            .flatten()
            .ok_or_else(|| PatcherError::NotFound(format!("method index {method_pos}")))?;
        let method_name = dex.string(dex.method_id(summary.method).name).to_string();
        Ok((class_desc, method_name))
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

fn ok_or_warn<T: Default>(dex_idx: usize, what: &str, result: DexResult<T>) -> T {
    result.unwrap_or_else(|error| {
        warn!(dex_idx, %error, operation = what, "dex scan failed");
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
        dex_idx,
        class_idx: site.class_idx,
        method_pos: site.method_pos,
        is_virtual: site.is_virtual,
        insn_idx: site.insn_idx,
    }
}

fn method_mut_at(dex: &mut DexFile, location: MethodLocation) -> Option<&mut EncodedMethod> {
    let data = dex.class_mut(location.class_idx).ok()?.class_data.as_mut()?;
    if location.is_virtual {
        data.virtual_methods.get_mut(location.method_idx)
    } else {
        data.direct_methods.get_mut(location.method_idx)
    }
}
