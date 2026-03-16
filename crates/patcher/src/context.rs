use stitch_apk::ApkFile;
use stitch_apk::stitch_dex::{
    ClassDef, DexFile, EncodedMethod, InstructionPattern, MethodMatch, MultiDexContainer,
};

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

    pub fn dex_mut(&mut self) -> &mut MultiDexContainer {
        self.apk.dex_mut()
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

    pub fn dex_count(&self) -> usize {
        self.apk.dex().len()
    }
}
