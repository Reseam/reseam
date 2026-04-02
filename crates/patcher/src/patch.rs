use crate::context::PatchContext;
use crate::error::Result;
use crate::options::OptionDeclaration;

#[derive(Debug, Clone)]
pub struct Compatibility {
    pub package: String,
    pub versions: Vec<String>,
}

impl Compatibility {
    pub fn package(package: impl Into<String>) -> Self {
        Self {
            package: package.into(),
            versions: Vec::new(),
        }
    }

    pub fn with_versions(package: impl Into<String>, versions: Vec<String>) -> Self {
        Self {
            package: package.into(),
            versions,
        }
    }
}

pub trait Patch: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str {
        ""
    }
    fn compatible_with(&self) -> &[Compatibility] {
        &[]
    }
    fn enabled_by_default(&self) -> bool {
        true
    }
    fn depends_on(&self) -> &[String] {
        &[]
    }
    fn options(&self) -> &[OptionDeclaration] {
        &[]
    }
    fn extension_dex(&self) -> &[String] {
        &[]
    }
    fn execute(&self, ctx: &mut PatchContext) -> Result<()>;
    fn after_dependents(&self, _ctx: &mut PatchContext) -> Result<()> {
        Ok(())
    }
}
