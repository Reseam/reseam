use crate::error::{invalid, Result};
use crate::patch::Patch;
use std::path::Path;

pub struct PatchBundle {
    pub name: String,
    pub patches: Vec<Box<dyn Patch>>,
}

impl PatchBundle {
    pub fn load(_path: impl AsRef<Path>) -> Result<Self> {
        Err(invalid(
            "patch bundle",
            "bundle loading not yet implemented",
        ))
    }
}
