use stitch_apk::stitch_dex::{DexFile, ClassDef};

/// The context passed to patches during execution.
pub struct PatchContext {
    pub dex_files: Vec<DexFile>,
}

impl PatchContext {
    pub fn new(dex_files: Vec<DexFile>) -> Self {
        Self { dex_files }
    }

    pub fn find_class(&self, descriptor: &str) -> Option<(usize, &ClassDef)> {
        for (i, dex) in self.dex_files.iter().enumerate() {
            if let Some(class) = dex.find_class(descriptor) {
                return Some((i, class));
            }
        }
        None
    }

    pub fn find_class_mut(&mut self, descriptor: &str) -> Option<(usize, &mut ClassDef)> {
        for (i, dex) in self.dex_files.iter_mut().enumerate() {
            if let Some(class) = dex.find_class_mut(descriptor) {
                return Some((i, class));
            }
        }
        None
    }
}
