use std::path::Path;

use stitch_apk::ApkFile;
use stitch_apk::axml::reader::AxmlDocument;
use stitch_apk::resources::arsc::ResourceTable;
use stitch_apk::stitch_dex::{
    ClassDef, DexFile, EncodedMethod, Instruction, InstructionPattern, MethodMatch,
    MultiDexContainer, ParseOptions, StringIdx,
};

use crate::error::{PatcherError, Result as PatcherResult};

pub struct PatchContext<'a> {
    apk: &'a mut ApkFile,
}

impl<'a> PatchContext<'a> {
    pub fn new(apk: &'a mut ApkFile) -> Self {
        Self { apk }
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

    pub fn find_class(&self, descriptor: &str) -> Option<(usize, &ClassDef)> {
        self.apk.dex().find_class(descriptor)
    }

    pub fn find_class_mut(&mut self, descriptor: &str) -> Option<(usize, &mut ClassDef)> {
        self.apk.dex_mut().find_class_mut(descriptor)
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
            .ok_or_else(|| {
                PatcherError::NotFound(format!("{class_descriptor}.{method_name}"))
            })
    }

    pub fn class_mut(&mut self, descriptor: &str) -> PatcherResult<(usize, &mut ClassDef)> {
        self.find_class_mut(descriptor)
            .ok_or_else(|| PatcherError::NotFound(format!("class {descriptor}")))
    }

    pub fn dex_mut(&mut self, index: usize) -> PatcherResult<&mut DexFile> {
        self.apk
            .dex_mut()
            .dex_mut(index)
            .ok_or_else(|| PatcherError::NotFound(format!("dex index {index}")))
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

    pub fn dex_file(&self, index: usize) -> Option<&DexFile> {
        self.apk.dex().dex(index)
    }

    pub fn dex_file_mut(&mut self, index: usize) -> Option<&mut DexFile> {
        self.apk.dex_mut().dex_mut(index)
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
            let dex = stitch_apk::stitch_dex::parse(&bytes, ParseOptions::default())
                .map_err(|e| PatcherError::Bundle {
                    reason: format!("failed to parse extension DEX {}: {e}", path.display()),
                })?;
            self.apk.dex_mut().add_dex(dex);
            count += 1;
        }
        Ok(count)
    }
}
