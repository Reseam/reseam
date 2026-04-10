use std::collections::{HashMap, HashSet};
use std::path::Path;

use stitch_apk::stitch_dex::{
    find_free_register, find_free_registers, ClassDef, CodeItem, DexFile, EncodedMethod,
    Fingerprint, FingerprintMatch, Instruction, InstructionPattern, MethodMatch, MultiDexContainer,
    ParseOptions, StringIdx,
};
use stitch_apk::ApkFile;
use stitch_apk::AxmlDocument;
use stitch_apk::ResourceTable;

use crate::error::{PatcherError, Result as PatcherResult};
use crate::log::{LogEntry, PatchLog};
use crate::options::PatchOptions;

#[derive(Debug, Clone, Copy)]
pub struct InstructionLocation {
    pub dex_idx: usize,
    pub class_idx: usize,
    pub method_idx: usize,
    pub insn_idx: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct MethodCallSiteHit {
    pub loc: InstructionLocation,
    pub target_index: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct FieldAccessSiteHit {
    pub loc: InstructionLocation,
    pub target_index: usize,
}

pub struct PatchContext<'a> {
    apk: &'a mut ApkFile,
    log: PatchLog,
    options: PatchOptions,
}

impl<'a> PatchContext<'a> {
    pub fn new(apk: &'a mut ApkFile) -> Self {
        Self {
            apk,
            log: PatchLog::default(),
            options: PatchOptions::default(),
        }
    }

    pub fn log(&mut self) -> &mut PatchLog {
        &mut self.log
    }

    pub fn set_log(&mut self, log: PatchLog) {
        self.log = log;
    }

    pub fn take_log_entries(&mut self) -> Vec<LogEntry> {
        self.log.take_entries()
    }

    pub fn options(&self) -> &PatchOptions {
        &self.options
    }

    pub fn set_options(&mut self, options: PatchOptions) {
        self.options = options;
    }

    pub fn clear_options(&mut self) {
        self.options = PatchOptions::default();
    }

    pub fn package_name(&self) -> Option<&str> {
        self.apk.package_name()
    }

    pub fn version_code(&self) -> Option<u32> {
        self.apk.version_code()
    }

    pub fn version_name(&self) -> Option<&str> {
        self.apk.version_name()
    }

    pub fn dex(&self) -> &MultiDexContainer {
        self.apk.dex()
    }

    pub fn dex_container_mut(&mut self) -> &mut MultiDexContainer {
        self.apk.dex_mut()
    }

    pub fn manifest(&self) -> &AxmlDocument {
        self.apk.manifest()
    }

    pub fn manifest_mut(&mut self) -> &mut AxmlDocument {
        self.apk.manifest_mut()
    }

    pub fn resources(&self) -> Option<&ResourceTable> {
        self.apk.resources()
    }

    pub fn resources_mut(&mut self) -> Option<&mut ResourceTable> {
        self.apk.resources_mut()
    }

    pub fn resource_component_names(&self) -> Vec<String> {
        (0..self.apk.component_count())
            .filter(|&index| self.apk.component_resources(index).is_some())
            .filter_map(|index| self.apk.component_meta(index).map(|component| component.name.clone()))
            .collect()
    }

    pub fn resource_component_name(&self, index: usize) -> Option<&str> {
        self.apk.component_meta(index).map(|component| component.name.as_str())
    }

    pub fn resource_component_index(&self, name: &str) -> Option<usize> {
        self.apk.component_index_by_name(name)
    }

    pub fn component_resources(&self, index: usize) -> Option<&ResourceTable> {
        self.apk.component_resources(index)
    }

    pub fn component_resources_mut(&mut self, index: usize) -> Option<&mut ResourceTable> {
        self.apk.component_resources_mut(index)
    }

    pub fn component_manifest(&self, index: usize) -> Option<&AxmlDocument> {
        self.apk.component_manifest(index)
    }

    pub fn component_manifest_mut(&mut self, index: usize) -> Option<&mut AxmlDocument> {
        self.apk.component_manifest_mut(index)
    }

    pub fn apk(&self) -> &ApkFile {
        self.apk
    }

    pub fn apk_mut(&mut self) -> &mut ApkFile {
        self.apk
    }

    pub fn component_names(&self) -> Vec<String> {
        (0..self.apk.component_count())
            .filter_map(|index| self.apk.component_meta(index).map(|component| component.name.clone()))
            .collect()
    }

    pub fn component_name(&self, index: usize) -> Option<&str> {
        self.apk.component_meta(index).map(|component| component.name.as_str())
    }

    pub fn component_index(&self, name: &str) -> Option<usize> {
        self.apk.component_index_by_name(name)
    }

    pub fn find_class(&self, descriptor: &str) -> Option<(usize, &ClassDef)> {
        self.apk.dex().find_class(descriptor)
    }

    pub fn find_class_mut(&mut self, descriptor: &str) -> Option<(usize, &mut ClassDef)> {
        self.apk.dex_mut().find_class_mut(descriptor)
    }

    pub fn class_mut(&mut self, descriptor: &str) -> PatcherResult<(usize, &mut ClassDef)> {
        self.find_class_mut(descriptor)
            .ok_or_else(|| PatcherError::NotFound(format!("class {descriptor}")))
    }

    pub fn find_method(
        &self,
        class_descriptor: &str,
        method_name: &str,
    ) -> Option<(usize, &EncodedMethod)> {
        for (i, dex) in self.apk.dex().iter().enumerate() {
            if let Some(class) = dex.find_class(class_descriptor) {
                if let Some(method) = class.find_method(method_name, &dex.methods, &dex.strings) {
                    return Some((i, method));
                }
            }
        }
        None
    }

    pub fn find_method_mut(
        &mut self,
        class_descriptor: &str,
        method_name: &str,
    ) -> Option<(usize, &mut EncodedMethod)> {
        let mut target = None;
        for (i, dex) in self.apk.dex().iter().enumerate() {
            for (ci, class) in dex.classes.iter().enumerate() {
                let type_desc = dex.type_descriptor(class.class_type);
                if type_desc == class_descriptor
                    && class
                        .find_method(method_name, &dex.methods, &dex.strings)
                        .is_some()
                {
                    target = Some((i, ci));
                    break;
                }
            }
            if target.is_some() {
                break;
            }
        }

        let (dex_idx, class_idx) = target?;
        let dex = self.apk.dex_mut().dex_mut(dex_idx)?;
        let class = &mut dex.classes[class_idx];
        class
            .find_method_mut(method_name, &dex.methods, &dex.strings)
            .map(|m| (dex_idx, m))
    }

    pub fn method_mut(
        &mut self,
        class_descriptor: &str,
        method_name: &str,
    ) -> PatcherResult<(usize, &mut EncodedMethod)> {
        self.find_method_mut(class_descriptor, method_name)
            .ok_or_else(|| PatcherError::NotFound(format!("{class_descriptor}.{method_name}")))
    }

    pub fn find_method_by_name(&self, method_name: &str) -> Option<(usize, MethodMatch<'_>)> {
        for (i, dex) in self.apk.dex().iter().enumerate() {
            let result = dex.find_method_by(|method_id, _class, _method| {
                dex.string(method_id.name) == method_name
            });
            if let Some(m) = result {
                return Some((i, m));
            }
        }
        None
    }

    pub fn find_methods_by_strings(&self, strings: &[&str]) -> Vec<(usize, MethodMatch<'_>)> {
        let mut results = Vec::new();
        for (i, dex) in self.apk.dex().iter().enumerate() {
            let string_idxs: Vec<StringIdx> = strings
                .iter()
                .filter_map(|s| dex.find_string_idx(s))
                .collect();
            if string_idxs.len() != strings.len() {
                continue;
            }
            let matches = dex.find_methods_by(|_method_id, _class, method| {
                let code = match &method.code {
                    Some(c) => c,
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
            for m in matches {
                results.push((i, m));
            }
        }
        results
    }

    pub fn find_methods_with_opcodes(
        &self,
        opcodes: &[InstructionPattern],
    ) -> Vec<(usize, MethodMatch<'_>)> {
        let mut results = Vec::new();
        for (i, dex) in self.apk.dex().iter().enumerate() {
            for m in dex.find_methods_with_opcodes(opcodes) {
                results.push((i, m));
            }
        }
        results
    }

    pub fn dex_file(&self, index: usize) -> Option<&DexFile> {
        self.apk.dex().dex(index)
    }

    pub fn dex_file_mut(&mut self, index: usize) -> Option<&mut DexFile> {
        self.apk.dex_mut().dex_mut(index)
    }

    pub fn dex_mut(&mut self, index: usize) -> PatcherResult<&mut DexFile> {
        self.apk
            .dex_mut()
            .dex_mut(index)
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
            let dex =
                stitch_apk::stitch_dex::parse(&bytes, ParseOptions::default()).map_err(|e| {
                    PatcherError::Bundle {
                        reason: format!("failed to parse extension DEX {}: {e}", path.display()),
                    }
                })?;
            self.apk.dex_mut().add_dex(dex);
            count += 1;
        }
        Ok(count)
    }

    pub fn find_method_by_fingerprint(
        &self,
        fp: &Fingerprint,
    ) -> Option<(usize, FingerprintMatch<'_>)> {
        for (i, dex) in self.apk.dex().iter().enumerate() {
            if let Some(m) = dex.find_method_by_fingerprint(fp) {
                return Some((i, m));
            }
        }
        None
    }

    pub fn find_methods_by_fingerprint(
        &self,
        fp: &Fingerprint,
    ) -> Vec<(usize, FingerprintMatch<'_>)> {
        let mut results = Vec::new();
        for (i, dex) in self.apk.dex().iter().enumerate() {
            for m in dex.find_methods_by_fingerprint(fp) {
                results.push((i, m));
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
        &self,
        mut predicate: impl FnMut(usize, &DexFile, &Instruction) -> bool,
    ) -> Vec<InstructionLocation> {
        let mut results = Vec::new();
        for (dex_idx, dex) in self.apk.dex().iter().enumerate() {
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

    pub fn find_instructions_by_literal(&self, literal: i64) -> Vec<InstructionLocation> {
        self.scan_instructions(|_, _, insn| insn.literal() == Some(literal))
    }

    pub fn find_instructions_by_string(&self, target: &str) -> Vec<InstructionLocation> {
        let idx_per_dex: Vec<Option<StringIdx>> = self
            .apk
            .dex()
            .iter()
            .map(|dex| dex.find_string_idx(target))
            .collect();
        self.scan_instructions(|dex_idx, _, insn| {
            idx_per_dex[dex_idx].is_some_and(|target_idx| insn.string_ref() == Some(target_idx))
        })
    }

    pub fn find_instructions_by_string_contains(
        &self,
        substring: &str,
    ) -> Vec<InstructionLocation> {
        let sets_per_dex: Vec<HashSet<StringIdx>> = self
            .apk
            .dex()
            .iter()
            .map(|dex| {
                dex.strings
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.value.contains(substring))
                    .map(|(i, _)| StringIdx(i as u32))
                    .collect()
            })
            .collect();
        self.scan_instructions(|dex_idx, _, insn| {
            insn.string_ref()
                .is_some_and(|sref| sets_per_dex[dex_idx].contains(&sref))
        })
    }

    /// Find all invoke instructions across all DEX files that call any of the given methods.
    /// Each target is (class_descriptor, method_name). Returns hits with a target index
    /// indicating which target matched.
    pub fn find_method_call_sites(&self, targets: &[(String, String)]) -> Vec<MethodCallSiteHit> {
        let mut results = Vec::new();
        for (dex_idx, dex) in self.apk.dex().iter().enumerate() {
            // Build a set of MethodIdx values that match any target
            let mut target_map: HashMap<stitch_apk::stitch_dex::MethodIdx, usize> = HashMap::new();
            for (target_idx, (class_desc, method_name)) in targets.iter().enumerate() {
                for (i, mid) in dex.methods.iter().enumerate() {
                    if dex.type_descriptor(mid.class) == *class_desc
                        && dex.string(mid.name) == *method_name
                    {
                        target_map.insert(stitch_apk::stitch_dex::MethodIdx(i as u32), target_idx);
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

    /// Find all field access instructions across all DEX files that reference any of the given fields.
    /// Each target is (defining_class, field_name). Returns hits with a target index.
    pub fn find_field_access_sites(&self, targets: &[(String, String)]) -> Vec<FieldAccessSiteHit> {
        let mut results = Vec::new();
        for (dex_idx, dex) in self.apk.dex().iter().enumerate() {
            let mut target_map: HashMap<stitch_apk::stitch_dex::FieldIdx, usize> = HashMap::new();
            for (target_idx, (class_desc, field_name)) in targets.iter().enumerate() {
                for (i, fid) in dex.fields.iter().enumerate() {
                    if dex.type_descriptor(fid.class) == *class_desc
                        && dex.string(fid.name) == *field_name
                    {
                        target_map.insert(stitch_apk::stitch_dex::FieldIdx(i as u32), target_idx);
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

    /// Extracts (class_descriptor, method_name) from a FingerprintMatch.
    pub fn resolve_fingerprint_location(
        &self,
        dex_idx: usize,
        fp_match: &FingerprintMatch<'_>,
    ) -> PatcherResult<(String, String)> {
        let dex = self
            .dex_file(dex_idx)
            .ok_or_else(|| PatcherError::NotFound(format!("dex {dex_idx}")))?;
        let class_desc = dex.type_descriptor(fp_match.class_idx).to_string();
        let method_id = &dex.methods[fp_match.method.method.0 as usize];
        let method_name = dex.string(method_id.name).to_string();
        Ok((class_desc, method_name))
    }

    /// Extracts (class_descriptor, method_name) from a MethodMatch.
    pub fn resolve_method_match_location(
        &self,
        dex_idx: usize,
        method_match: &MethodMatch<'_>,
    ) -> PatcherResult<(String, String)> {
        let dex = self
            .dex_file(dex_idx)
            .ok_or_else(|| PatcherError::NotFound(format!("dex {dex_idx}")))?;
        let class_desc = dex.type_descriptor(method_match.class_idx).to_string();
        let method_id = &dex.methods[method_match.method.method.0 as usize];
        let method_name = dex.string(method_id.name).to_string();
        Ok((class_desc, method_name))
    }

    /// Extracts (class_descriptor, method_name) from an instruction location.
    pub fn resolve_literal_location(
        &self,
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
        let data = class
            .class_data
            .as_ref()
            .ok_or_else(|| PatcherError::NotFound("class data".to_string()))?;
        let method = data
            .direct_methods
            .iter()
            .chain(&data.virtual_methods)
            .nth(loc.method_idx)
            .ok_or_else(|| PatcherError::NotFound(format!("method index {}", loc.method_idx)))?;
        let method_id = &dex.methods[method.method.0 as usize];
        let method_name = dex.string(method_id.name).to_string();
        Ok((class_desc, method_name))
    }

    pub fn find_resource_id(&self, type_name: &str, entry_name: &str) -> Option<u32> {
        self.apk.find_resource_id(type_name, entry_name)
    }

    pub fn find_resource_component(&self, type_name: &str, entry_name: &str) -> Option<usize> {
        self.apk.find_resource_component(type_name, entry_name)
    }

    pub fn find_resource_component_by_id(&self, res_id: u32) -> Option<usize> {
        self.apk.find_resource_component_by_id(res_id)
    }

    pub fn find_resource_id_in_component(
        &self,
        component_index: usize,
        type_name: &str,
        entry_name: &str,
    ) -> Option<u32> {
        self.apk
            .component_find_resource_id(component_index, type_name, entry_name)
    }

    pub fn resource_exists(&self, type_name: &str, entry_name: &str) -> bool {
        self.apk.resource_exists(type_name, entry_name)
    }

    pub fn get_string_resource_value(&self, name: &str) -> Option<&str> {
        self.apk.get_string_resource_value(name)
    }

    pub fn set_string_resource_value(&mut self, name: &str, value: &str) -> bool {
        self.apk.set_string_resource_value(name, value)
    }

    pub fn inject_file(&mut self, apk_path: &str, data: Vec<u8>) {
        let Some(data) = self.auto_compile_xml(apk_path, data) else {
            return;
        };
        if apk_path == "AndroidManifest.xml" {
            match stitch_apk::AxmlDocument::parse(&data) {
                Ok(document) => {
                    *self.apk.manifest_mut() = document;
                }
                Err(e) => {
                    self.log().warn(format!(
                        "inject_file: failed to parse compiled manifest {apk_path}: {e}"
                    ));
                }
            }
            return;
        }
        self.apk.inject_file(apk_path, data);
    }

    pub fn inject_file_into_component(
        &mut self,
        component_index: usize,
        apk_path: &str,
        data: Vec<u8>,
    ) {
        let Some(data) = self.auto_compile_xml_in_component(component_index, apk_path, data) else {
            return;
        };
        if apk_path == "AndroidManifest.xml" {
            match stitch_apk::AxmlDocument::parse(&data) {
                Ok(document) => {
                    if let Some(manifest) = self.apk.component_manifest_mut(component_index) {
                        *manifest = document;
                    }
                }
                Err(e) => {
                    self.log().warn(format!(
                        "inject_file_into_component: failed to parse compiled manifest {apk_path}: {e}"
                    ));
                }
            }
            return;
        }
        self.apk.inject_file_into(component_index, apk_path, data);
    }

    fn auto_compile_xml(&mut self, apk_path: &str, data: Vec<u8>) -> Option<Vec<u8>> {
        if !apk_path.ends_with(".xml") {
            return Some(data);
        }
        if stitch_apk::axml::compiler::is_compiled_axml(&data) {
            return Some(data);
        }
        match std::str::from_utf8(&data) {
            Ok(text) => {
                let result = stitch_apk::axml::compiler::compile_xml_with_resources(
                    text,
                    self.apk.resources_mut(),
                );
                match result {
                    Ok(compiled) => Some(compiled),
                    Err(e) if requires_compiled_xml(apk_path) => {
                        self.log().warn(format!(
                            "inject_file: refused to inject plain XML at {apk_path}: {e}"
                        ));
                        None
                    }
                    Err(_) => Some(data),
                }
            }
            Err(_) if requires_compiled_xml(apk_path) => {
                self.log().warn(format!(
                    "inject_file: refused to inject non-UTF8 XML at {apk_path}"
                ));
                None
            }
            Err(_) => Some(data),
        }
    }

    fn auto_compile_xml_in_component(
        &mut self,
        component_index: usize,
        apk_path: &str,
        data: Vec<u8>,
    ) -> Option<Vec<u8>> {
        if !apk_path.ends_with(".xml") {
            return Some(data);
        }
        if stitch_apk::axml::compiler::is_compiled_axml(&data) {
            return Some(data);
        }
        match std::str::from_utf8(&data) {
            Ok(text) => {
                let result = stitch_apk::axml::compiler::compile_xml_with_resources(
                    text,
                    self.apk.component_resources_mut(component_index),
                );
                match result {
                    Ok(compiled) => Some(compiled),
                    Err(e) if requires_compiled_xml(apk_path) => {
                        self.log().warn(format!(
                            "inject_file_into_component: refused to inject plain XML at {apk_path}: {e}"
                        ));
                        None
                    }
                    Err(_) => Some(data),
                }
            }
            Err(_) if requires_compiled_xml(apk_path) => {
                self.log().warn(format!(
                    "inject_file_into_component: refused to inject non-UTF8 XML at {apk_path}"
                ));
                None
            }
            Err(_) => Some(data),
        }
    }

    pub fn inject_file_stored(&mut self, apk_path: &str, data: Vec<u8>) {
        self.apk.inject_file_stored(apk_path, data);
    }

    pub fn inject_file_stored_into_component(
        &mut self,
        component_index: usize,
        apk_path: &str,
        data: Vec<u8>,
    ) {
        self.apk
            .inject_file_stored_into(component_index, apk_path, data);
    }

    pub fn delete_file(&mut self, apk_path: &str) {
        self.apk.delete_file(apk_path);
    }

    pub fn delete_file_from_component(&mut self, component_index: usize, apk_path: &str) {
        self.apk.delete_file_from(component_index, apk_path);
    }

    pub fn list_files(&self) -> &[String] {
        self.apk.entry_names()
    }

    pub fn list_files_in_component(&self, component_index: usize) -> Option<&[String]> {
        self.apk.component_entry_names(component_index)
    }

    pub fn read_file(&self, apk_path: &str) -> Option<Vec<u8>> {
        self.apk.read_entry(apk_path).ok()
    }

    pub fn read_file_from_component(&self, component_index: usize, apk_path: &str) -> Option<Vec<u8>> {
        self.apk.read_entry_from_component(component_index, apk_path).ok()
    }

    pub fn copy_resource_group(
        &mut self,
        bundle_dir: &Path,
        res_type: &str,
        files: &[&str],
    ) -> PatcherResult<usize> {
        let mut count = 0;
        for file_name in files {
            let src = bundle_dir.join("resources").join(res_type).join(file_name);
            if !src.exists() {
                return Err(PatcherError::Bundle {
                    reason: format!("missing resource file {}", src.display()),
                });
            }
            let data = std::fs::read(&src)?;
            let apk_path = format!("res/{res_type}/{file_name}");
            self.inject_file(&apk_path, data);
            count += 1;
        }
        Ok(count)
    }
}

fn requires_compiled_xml(apk_path: &str) -> bool {
    apk_path == "AndroidManifest.xml" || apk_path.starts_with("res/")
}
