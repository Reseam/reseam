use std::path::Path;
use crate::error::Result;
use crate::patch::Patch;

pub fn load_lua_patch(_path: impl AsRef<Path>) -> Result<Box<dyn Patch>> {
    todo!("Lua patch loading not yet implemented")
}
