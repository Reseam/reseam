use crate::context::PatchContext;
use crate::error::Result;

/// The core trait that all patches implement, whether native Rust or scripted.
pub trait Patch: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn compatible_packages(&self) -> &[&str];
    fn enabled_by_default(&self) -> bool { true }
    fn execute(&self, ctx: &mut PatchContext) -> Result<()>;
}
