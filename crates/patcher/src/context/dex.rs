// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::path::Path;

use reseam_apk::reseam_dex::{
    find_free_register, find_free_registers, ClassDef, CodeItem, DexFile, EncodedField,
    EncodedMethod, Fingerprint, FingerprintHit, InstructionPattern, InstructionSite, MemberCounts,
    MethodHit, MethodIdx, MultiDexContainer, ParseOptions, StringIdx,
};
use tracing::warn;

use super::{
    ClassLocation, FieldAccessSiteHit, FingerprintLocation, InstructionLocation, MethodCallSiteHit,
    MethodLocation, PatchContext,
};
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

    /// Reads one method for inspection without materializing its class. Returns
    /// the (non-resolving) DEX plus an owned decode of the method, so read-only
    /// FFIs never persist a class's IR just to look at one of its methods.
    pub fn read_method(
        &self,
        dex_idx: usize,
        class_idx: usize,
        method_pos: usize,
        is_virtual: bool,
    ) -> Option<(&DexFile, EncodedMethod)> {
        let dex = self.dex_file(dex_idx)?;
        let method = dex
            .decode_method_at(class_idx, method_pos, is_virtual)
            .ok()
            .flatten()?;
        Some((dex, method))
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
        let class = dex.classes.get_mut(location.class_idx)?;
        Some((location, class))
    }

    pub fn class_mut(&mut self, descriptor: &str) -> PatcherResult<(ClassLocation, &mut ClassDef)> {
        self.find_class_mut(descriptor)
            .ok_or_else(|| PatcherError::NotFound(format!("class {descriptor}")))
    }

    pub fn find_method(
        &mut self,
        class_descriptor: &str,
        method_name: &str,
    ) -> Option<MethodLocation> {
        for dex_idx in 0..self.dex_count() {
            let class_idx = {
                let dex = self.apk.dex().dex(dex_idx)?;
                dex.find_class_index(class_descriptor)
            };
            let Some(class_idx) = class_idx else { continue };

            let dex = self.class_dex(dex_idx, class_idx)?;
            let class = dex.classes.get(class_idx)?;
            if let Some((method_idx, is_virtual)) = find_method_slot(class, dex, method_name) {
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

    pub fn find_method_by_name(&self, method_name: &str) -> Option<MethodLocation> {
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

    pub fn all_methods(&self) -> Vec<MethodLocation> {
        let mut results = Vec::new();
        for dex_idx in 0..self.dex_count() {
            let Some(dex) = self.dex_file(dex_idx) else {
                continue;
            };
            let hits = ok_or_warn(
                dex_idx,
                "all_methods",
                dex.scan_methods_collect(|view| Ok(Some(view.hit()))),
            );
            results.extend(hits.iter().map(|hit| method_location(dex_idx, hit)));
        }
        results
    }

    pub fn find_methods_by_fingerprint(&self, fp: &Fingerprint) -> Vec<FingerprintLocation> {
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
        self.scan_instructions("find_instructions_by_literal", |dex_idx, _dex, site| {
            (site.instruction.literal() == Some(literal)).then(|| instruction_location(dex_idx, site))
        })
    }

    pub fn find_instructions_by_string(&self, target: &str) -> Vec<InstructionLocation> {
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
                dex.scan_instructions(|site| {
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
        let mut results = Vec::new();
        for dex_idx in 0..self.dex_count() {
            let Some(dex) = self.dex_file(dex_idx) else {
                continue;
            };
            let matches: Vec<StringIdx> = dex
                .strings
                .iter()
                .enumerate()
                .filter(|(_, s)| s.value.contains(substring))
                .map(|(i, _)| StringIdx(i as u32))
                .collect();
            if matches.is_empty() {
                continue;
            }
            let hits = ok_or_warn(
                dex_idx,
                "find_instructions_by_string_contains",
                dex.scan_instructions(|site| {
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
        let mut results = Vec::new();
        for dex_idx in 0..self.dex_count() {
            let Some(dex) = self.dex_file(dex_idx) else {
                continue;
            };
            let mut target_map: HashMap<MethodIdx, usize> = HashMap::new();
            for (target_idx, (class_desc, method_name)) in targets.iter().enumerate() {
                for (i, mid) in dex.methods.iter().enumerate() {
                    if dex.type_descriptor(mid.class) == *class_desc
                        && dex.string(mid.name) == *method_name
                    {
                        target_map.insert(MethodIdx(i as u32), target_idx);
                    }
                }
            }
            if target_map.is_empty() {
                continue;
            }
            let hits = ok_or_warn(
                dex_idx,
                "find_method_call_sites",
                dex.scan_instructions(|site| {
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
        let mut results = Vec::new();
        for dex_idx in 0..self.dex_count() {
            let Some(dex) = self.dex_file(dex_idx) else {
                continue;
            };
            let mut target_map: HashMap<reseam_apk::reseam_dex::FieldIdx, usize> = HashMap::new();
            for (target_idx, (class_desc, field_name)) in targets.iter().enumerate() {
                for (i, fid) in dex.fields.iter().enumerate() {
                    if dex.type_descriptor(fid.class) == *class_desc
                        && dex.string(fid.name) == *field_name
                    {
                        target_map.insert(reseam_apk::reseam_dex::FieldIdx(i as u32), target_idx);
                    }
                }
            }
            if target_map.is_empty() {
                continue;
            }
            let hits = ok_or_warn(
                dex_idx,
                "find_field_access_sites",
                dex.scan_instructions(|site| {
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
        matcher: impl Fn(usize, &DexFile, &InstructionSite<'_>) -> Option<T> + Sync,
    ) -> Vec<T> {
        let mut results = Vec::new();
        for dex_idx in 0..self.dex_count() {
            let Some(dex) = self.dex_file(dex_idx) else {
                continue;
            };
            let hits = ok_or_warn(
                dex_idx,
                what,
                dex.scan_instructions(|site| matcher(dex_idx, dex, site)),
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
        let dex = self
            .class_dex(location.dex_idx, location.class_idx)
            .ok_or_else(|| PatcherError::NotFound(format!("dex {}", location.dex_idx)))?;
        let class = dex
            .classes
            .get(location.class_idx)
            .ok_or_else(|| PatcherError::NotFound(format!("class index {}", location.class_idx)))?;
        let class_desc = dex.type_descriptor(class.class_type).to_string();
        let method = method_ref_at(dex, location).ok_or_else(|| {
            PatcherError::NotFound(format!("method index {}", location.method_idx))
        })?;
        let method_id = &dex.methods[method.method.0 as usize];
        let method_name = dex.string(method_id.name).to_string();
        Ok((class_desc, method_name))
    }

    pub fn resolve_literal_location(
        &mut self,
        loc: &InstructionLocation,
    ) -> PatcherResult<(String, String)> {
        let dex = self
            .class_dex(loc.dex_idx, loc.class_idx)
            .ok_or_else(|| PatcherError::NotFound(format!("dex {}", loc.dex_idx)))?;
        let class = dex
            .classes
            .get(loc.class_idx)
            .ok_or_else(|| PatcherError::NotFound(format!("class index {}", loc.class_idx)))?;
        let class_desc = dex.type_descriptor(class.class_type).to_string();
        let method = method_at(class, loc.method_pos, loc.is_virtual)
            .ok_or_else(|| PatcherError::NotFound(format!("method index {}", loc.method_pos)))?;
        let method_id = &dex.methods[method.method.0 as usize];
        let method_name = dex.string(method_id.name).to_string();
        Ok((class_desc, method_name))
    }
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

fn find_method_slot(class: &ClassDef, dex: &DexFile, method_name: &str) -> Option<(usize, bool)> {
    let data = class.class_data.as_ref()?;

    if let Some(method_idx) = data.direct_methods.iter().position(|method| {
        let method_id = &dex.methods[method.method.0 as usize];
        dex.string(method_id.name) == method_name
    }) {
        return Some((method_idx, false));
    }

    data.virtual_methods
        .iter()
        .position(|method| {
            let method_id = &dex.methods[method.method.0 as usize];
            dex.string(method_id.name) == method_name
        })
        .map(|method_idx| (method_idx, true))
}

fn method_at(class: &ClassDef, method_pos: usize, is_virtual: bool) -> Option<&EncodedMethod> {
    let data = class.class_data.as_ref()?;
    if is_virtual {
        data.virtual_methods.get(method_pos)
    } else {
        data.direct_methods.get(method_pos)
    }
}

fn method_ref_at(dex: &DexFile, location: MethodLocation) -> Option<&EncodedMethod> {
    method_at(dex.classes.get(location.class_idx)?, location.method_idx, location.is_virtual)
}

fn method_mut_at(dex: &mut DexFile, location: MethodLocation) -> Option<&mut EncodedMethod> {
    let class = dex.classes.get_mut(location.class_idx)?;
    let data = class.class_data.as_mut()?;
    if location.is_virtual {
        data.virtual_methods.get_mut(location.method_idx)
    } else {
        data.direct_methods.get_mut(location.method_idx)
    }
}
