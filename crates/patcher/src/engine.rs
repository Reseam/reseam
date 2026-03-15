use crate::context::PatchContext;
use crate::error::Result;
use crate::patch::Patch;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchEvent {
    Applying { name: String },
    Applied { name: String },
}

/// Apply a list of patches to a PatchContext.
pub fn apply_patches(
    ctx: &mut PatchContext,
    patches: &[Box<dyn Patch>],
) -> Result<Vec<PatchEvent>> {
    let mut events = Vec::with_capacity(patches.len() * 2);
    for patch in patches {
        let name = patch.name().to_owned();
        events.push(PatchEvent::Applying { name: name.clone() });
        patch.execute(ctx)?;
        events.push(PatchEvent::Applied { name });
    }
    Ok(events)
}
