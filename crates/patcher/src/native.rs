use std::path::Path;
use crate::error::Result;
use crate::patch::Patch;

pub fn load_native_patch(_path: impl AsRef<Path>) -> Result<Box<dyn Patch>> {
    todo!("Native patch loading not yet implemented")
}
