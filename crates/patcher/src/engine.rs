use crate::error::Result;
use crate::patch::Patch;
use crate::context::PatchContext;

/// Apply a list of patches to a PatchContext.
pub fn apply_patches(ctx: &mut PatchContext, patches: &[Box<dyn Patch>]) -> Result<()> {
    for patch in patches {
        eprintln!("[stitch] Applying: {}", patch.name());
        patch.execute(ctx)?;
        eprintln!("[stitch] Done: {}", patch.name());
    }
    Ok(())
}
