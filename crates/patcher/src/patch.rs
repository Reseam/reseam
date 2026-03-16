use crate::context::PatchContext;
use crate::error::Result;

pub trait Patch: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn compatible_packages(&self) -> &[String];
    fn compatible_versions(&self) -> &[String];
    fn enabled_by_default(&self) -> bool {
        true
    }
    fn execute(&self, ctx: &mut PatchContext) -> Result<()>;
}
