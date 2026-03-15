use std::path::Path;
use crate::error::{Result, PatcherError};
use crate::patch::Patch;

pub struct PatchBundle {
    pub name: String,
    pub patches: Vec<Box<dyn Patch>>,
}

impl PatchBundle {
    pub fn load(_path: impl AsRef<Path>) -> Result<Self> {
        Err(PatcherError::BundleError {
            reason: "Bundle loading not yet implemented".into(),
        })
    }
}
