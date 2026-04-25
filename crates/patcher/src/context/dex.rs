// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{HashMap, HashSet};
use std::path::Path;

use reseam_apk::reseam_dex::{
    find_free_register, find_free_registers, ClassDef, CodeItem, DexFile, EncodedMethod,
    Fingerprint, Instruction, InstructionPattern, MultiDexContainer, ParseOptions, StringIdx,
};

use super::{
    ClassLocation, FieldAccessSiteHit, FingerprintLocation, InstructionLocation, MethodCallSiteHit,
    MethodLocation, PatchContext,
};
use crate::error::{PatcherError, Result as PatcherResult};

impl<'a> PatchContext<'a> {
    pub fn dex(&self) -> &MultiDexContainer {
        self.apk.dex()
    }

    pub fn dex_container_mut(&mut self) -> reseam_apk::apk_file::ApkDexMut<'_> {
        self.apk.dex_mut()
    }

    pub fn find_class(&self, descriptor: &str) -> Option<ClassLocation> {
        let (dex_idx, class) = self.apk.dex().find_class(descriptor)?;
        let dex = self.apk.dex().dex(dex_idx)?;
        let class_idx = locate_class_ref(dex, class)?;
        Some(ClassLocation { dex_idx, class_idx })
    }

    pub fn find_class_mut(&mut self, descriptor: &str) -> Option<(ClassLocation, &mut ClassDef)> {
        let mut target = None;
        for dex_idx in 0..self.dex_count() {
            let Some(dex) = self.apk.dex().dex(dex_idx) else {
                continue;
            };
            if let Some(class_idx) = dex.find_class_index(descriptor) {
                target = Some(ClassLocation { dex_idx, class_idx });
                break;
            }
        }

        let location = target?;
        let dex = self.dex_file_mut(location.dex_idx)?;
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
            let dex = self.dex_file(dex_idx)?;
            let Some(class_idx) = dex.find_class_index(class_descriptor) else {
                continue;
            };
            let class = dex.classes.get(class_idx)?;
            let Some((method_idx, is_virtual)) = find_method_slot(class, dex, method_name) else {
                continue;
            };
            return Some(MethodLocation {
                dex_idx,
                class_idx,
                method_idx,
                is_virtual,
            });
        }
        None
    }

    pub fn find_method_mut(
        &mut self,
        class_descriptor: &str,
        method_name: &str,
    ) -> Option<(MethodLocation, &mut EncodedMethod)> {
        let location = self.find_method(class_descriptor, method_name)?;
        let dex = self.dex_file_mut(location.dex_idx)?;
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

    pub fn find_method_by_name(&mut self, method_name: &str) -> Option<MethodLocation> {
        for dex_idx in 0..self.dex_count() {
            let dex = self.dex_file(dex_idx)?;
            let result = dex.find_method_by(|method_id, _class, _method| {
                dex.string(method_id.name) == method_name
            });
            let Some(method_match) = result else {
                continue;
            };
            let (class_idx, method_idx, is_virtual) = locate_method_ref(dex, method_match.method)?;
            return Some(MethodLocation {
                dex_idx,
                class_idx,
                method_idx,
                is_virtual,
            });
        }
        None
    }

    pub fn find_methods_by_strings(&mut self, strings: &[&str]) -> Vec<MethodLocation> {
        let mut results = Vec::new();
        for dex_idx in 0..self.dex_count() {
            let Some(dex) = self.dex_file(dex_idx) else {
                continue;
            };
            let string_idxs: Vec<StringIdx> = strings
                .iter()
                .filter_map(|s| dex.find_string_idx(s))
                .collect();
            if string_idxs.len() != strings.len() {
                continue;
            }

            let matches = dex.find_methods_by(|_method_id, _class, method| {
                let code = match &method.code {
                    Some(code) => code,
                    None => return false,
                };
                string_idxs.iter().all(|target| {
                    code.instructions.iter().any(|insn| match insn {
                        Instruction::ConstString { string, .. }
                        | Instruction::ConstStringJumbo { string, .. } => string == target,
                        _ => false,
                    })
                })
            });

            for method_match in matches {
                if let Some((class_idx, method_idx, is_virtual)) =
                    locate_method_ref(dex, method_match.method)
                {
                    results.push(MethodLocation {
                        dex_idx,
                        class_idx,
                        method_idx,
                        is_virtual,
                    });
                }
            }
        }
        results
    }

    pub fn find_methods_with_opcodes(
        &mut self,
        opcodes: &[InstructionPattern],
    ) -> Vec<MethodLocation> {
        let mut results = Vec::new();
        for dex_idx in 0..self.dex_count() {
            let Some(dex) = self.dex_file(dex_idx) else {
                continue;
            };
            for method_match in dex.find_methods_with_opcodes(opcodes) {
                if let Some((class_idx, method_idx, is_virtual)) =
                    locate_method_ref(dex, method_match.method)
                {
                    results.push(MethodLocation {
                        dex_idx,
                        class_idx,
                        method_idx,
                        is_virtual,
                    });
                }
            }
        }
        results
    }

    pub fn dex_file(&mut self, index: usize) -> Option<&DexFile> {
        self.apk.resolved_dex(index).ok().flatten()
    }

    pub fn dex_file_mut(&mut self, index: usize) -> Option<&mut DexFile> {
        self.apk.resolved_dex_mut(index).ok().flatten()
    }

    pub fn dex_mut(&mut self, index: usize) -> PatcherResult<&mut DexFile> {
        self.dex_file_mut(index)
            .ok_or_else(|| PatcherError::NotFound(format!("dex index {index}")))
    }

    pub fn dex_count(&self) -> usize {
        self.apk.dex().len()
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

    pub fn find_method_by_fingerprint(&mut self, fp: &Fingerprint) -> Option<FingerprintLocation> {
        for dex_idx in 0..self.dex_count() {
            let dex = self.dex_file(dex_idx)?;
            let Some(fingerprint_match) = dex.find_method_by_fingerprint(fp) else {
                continue;
            };
            let (class_idx, method_idx, is_virtual) =
                locate_method_ref(dex, fingerprint_match.method)?;
            return Some(FingerprintLocation {
                method: MethodLocation {
                    dex_idx,
                    class_idx,
                    method_idx,
                    is_virtual,
                },
                matched_indices: fingerprint_match.matched_indices.clone(),
            });
        }
        None
    }

    pub fn find_methods_by_fingerprint(&mut self, fp: &Fingerprint) -> Vec<FingerprintLocation> {
        let mut results = Vec::new();
        for dex_idx in 0..self.dex_count() {
            let Some(dex) = self.dex_file(dex_idx) else {
                continue;
            };
            for fingerprint_match in dex.find_methods_by_fingerprint(fp) {
                if let Some((class_idx, method_idx, is_virtual)) =
                    locate_method_ref(dex, fingerprint_match.method)
                {
                    results.push(FingerprintLocation {
                        method: MethodLocation {
                            dex_idx,
                            class_idx,
                            method_idx,
                            is_virtual,
                        },
                        matched_indices: fingerprint_match.matched_indices.clone(),
                    });
                }
            }
        }
        results
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

    fn scan_instructions(
        &mut self,
        mut predicate: impl FnMut(usize, &DexFile, &Instruction) -> bool,
    ) -> Vec<InstructionLocation> {
        let mut results = Vec::new();
        for dex_idx in 0..self.dex_count() {
            let Some(dex) = self.dex_file(dex_idx) else {
                continue;
            };
            for (class_idx, class) in dex.classes.iter().enumerate() {
                if let Some(data) = &class.class_data {
                    for (method_idx, method) in data
                        .direct_methods
                        .iter()
                        .chain(&data.virtual_methods)
                        .enumerate()
                    {
                        if let Some(code) = &method.code {
                            for (insn_idx, insn) in code.instructions.iter().enumerate() {
                                if predicate(dex_idx, dex, insn) {
                                    results.push(InstructionLocation {
                                        dex_idx,
                                        class_idx,
                                        method_idx,
                                        insn_idx,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        results
    }

    pub fn find_instructions_by_literal(&mut self, literal: i64) -> Vec<InstructionLocation> {
        self.scan_instructions(|_, _, insn| insn.literal() == Some(literal))
    }

    pub fn find_instructions_by_string(&mut self, target: &str) -> Vec<InstructionLocation> {
        let mut idx_per_dex = Vec::with_capacity(self.dex_count());
        for dex_idx in 0..self.dex_count() {
            idx_per_dex.push(
                self.dex_file(dex_idx)
                    .and_then(|dex| dex.find_string_idx(target)),
            );
        }
        self.scan_instructions(|dex_idx, _, insn| {
            idx_per_dex[dex_idx].is_some_and(|target_idx| insn.string_ref() == Some(target_idx))
        })
    }

    pub fn find_instructions_by_string_contains(
        &mut self,
        substring: &str,
    ) -> Vec<InstructionLocation> {
        let mut sets_per_dex: Vec<HashSet<StringIdx>> = Vec::with_capacity(self.dex_count());
        for dex_idx in 0..self.dex_count() {
            let matches = self
                .dex_file(dex_idx)
                .map(|dex| {
                    dex.strings
                        .iter()
                        .enumerate()
                        .filter(|(_, s)| s.value.contains(substring))
                        .map(|(i, _)| StringIdx(i as u32))
                        .collect()
                })
                .unwrap_or_default();
            sets_per_dex.push(matches);
        }
        self.scan_instructions(|dex_idx, _, insn| {
            insn.string_ref()
                .is_some_and(|sref| sets_per_dex[dex_idx].contains(&sref))
        })
    }

    pub fn find_method_call_sites(
        &mut self,
        targets: &[(String, String)],
    ) -> Vec<MethodCallSiteHit> {
        let mut results = Vec::new();
        for dex_idx in 0..self.dex_count() {
            let Some(dex) = self.dex_file(dex_idx) else {
                continue;
            };
            let mut target_map: HashMap<reseam_apk::reseam_dex::MethodIdx, usize> = HashMap::new();
            for (target_idx, (class_desc, method_name)) in targets.iter().enumerate() {
                for (i, mid) in dex.methods.iter().enumerate() {
                    if dex.type_descriptor(mid.class) == *class_desc
                        && dex.string(mid.name) == *method_name
                    {
                        target_map.insert(reseam_apk::reseam_dex::MethodIdx(i as u32), target_idx);
                    }
                }
            }
            if target_map.is_empty() {
                continue;
            }
            for (class_idx, class) in dex.classes.iter().enumerate() {
                if let Some(data) = &class.class_data {
                    for (method_idx, method) in data
                        .direct_methods
                        .iter()
                        .chain(&data.virtual_methods)
                        .enumerate()
                    {
                        if let Some(code) = &method.code {
                            for (insn_idx, insn) in code.instructions.iter().enumerate() {
                                if let Some(mr) = insn.method_ref() {
                                    if let Some(&target_idx) = target_map.get(&mr) {
                                        results.push(MethodCallSiteHit {
                                            loc: InstructionLocation {
                                                dex_idx,
                                                class_idx,
                                                method_idx,
                                                insn_idx,
                                            },
                                            target_index: target_idx,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        results
    }

    pub fn find_field_access_sites(
        &mut self,
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
            for (class_idx, class) in dex.classes.iter().enumerate() {
                if let Some(data) = &class.class_data {
                    for (method_idx, method) in data
                        .direct_methods
                        .iter()
                        .chain(&data.virtual_methods)
                        .enumerate()
                    {
                        if let Some(code) = &method.code {
                            for (insn_idx, insn) in code.instructions.iter().enumerate() {
                                if let Some(fr) = insn.field_ref() {
                                    if let Some(&target_idx) = target_map.get(&fr) {
                                        results.push(FieldAccessSiteHit {
                                            loc: InstructionLocation {
                                                dex_idx,
                                                class_idx,
                                                method_idx,
                                                insn_idx,
                                            },
                                            target_index: target_idx,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
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
            .dex_file(location.dex_idx)
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
            .dex_file(loc.dex_idx)
            .ok_or_else(|| PatcherError::NotFound(format!("dex {}", loc.dex_idx)))?;
        let class = dex
            .classes
            .get(loc.class_idx)
            .ok_or_else(|| PatcherError::NotFound(format!("class index {}", loc.class_idx)))?;
        let class_desc = dex.type_descriptor(class.class_type).to_string();
        let method = method_ref_from_combined_index(class, loc.method_idx)
            .ok_or_else(|| PatcherError::NotFound(format!("method index {}", loc.method_idx)))?;
        let method_id = &dex.methods[method.method.0 as usize];
        let method_name = dex.string(method_id.name).to_string();
        Ok((class_desc, method_name))
    }
}

fn locate_class_ref(dex: &DexFile, class: &ClassDef) -> Option<usize> {
    dex.classes
        .iter()
        .position(|candidate| std::ptr::eq(candidate, class))
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

fn locate_method_ref(dex: &DexFile, method: &EncodedMethod) -> Option<(usize, usize, bool)> {
    for (class_idx, class) in dex.classes.iter().enumerate() {
        let Some(data) = class.class_data.as_ref() else {
            continue;
        };

        for (method_idx, candidate) in data.direct_methods.iter().enumerate() {
            if std::ptr::eq(candidate, method) {
                return Some((class_idx, method_idx, false));
            }
        }

        for (method_idx, candidate) in data.virtual_methods.iter().enumerate() {
            if std::ptr::eq(candidate, method) {
                return Some((class_idx, method_idx, true));
            }
        }
    }

    None
}

fn method_ref_at(dex: &DexFile, location: MethodLocation) -> Option<&EncodedMethod> {
    let class = dex.classes.get(location.class_idx)?;
    let data = class.class_data.as_ref()?;
    if location.is_virtual {
        data.virtual_methods.get(location.method_idx)
    } else {
        data.direct_methods.get(location.method_idx)
    }
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

fn method_ref_from_combined_index(class: &ClassDef, method_idx: usize) -> Option<&EncodedMethod> {
    let data = class.class_data.as_ref()?;
    if method_idx < data.direct_methods.len() {
        data.direct_methods.get(method_idx)
    } else {
        data.virtual_methods
            .get(method_idx.saturating_sub(data.direct_methods.len()))
    }
}
