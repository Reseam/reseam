use crate::context::PatchContext;
use crate::error::{PatcherError, Result};
use crate::patch::Patch;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchResult {
    Applied { name: String },
    Skipped { name: String, reason: String },
}

pub fn apply_patches(
    ctx: &mut PatchContext,
    patches: &[Box<dyn Patch>],
) -> Result<Vec<PatchResult>> {
    let package = ctx.package_name().map(|s| s.to_owned());
    let version = ctx.version_name().map(|s| s.to_owned());
    let mut results = Vec::with_capacity(patches.len());

    for patch in patches {
        if let Some(reason) = check_compatibility(patch.as_ref(), &package, &version) {
            results.push(PatchResult::Skipped {
                name: patch.name().to_owned(),
                reason,
            });
            continue;
        }

        patch.execute(ctx).map_err(|e| PatcherError::PatchFailed {
            name: patch.name().to_owned(),
            reason: e.to_string(),
        })?;

        results.push(PatchResult::Applied {
            name: patch.name().to_owned(),
        });
    }

    Ok(results)
}

fn check_compatibility(
    patch: &dyn Patch,
    package: &Option<String>,
    version: &Option<String>,
) -> Option<String> {
    let packages = patch.compatible_packages();
    if !packages.is_empty() {
        match package {
            Some(pkg) if !packages.iter().any(|p| p == pkg) => {
                return Some(format!("incompatible package: {pkg}"));
            }
            None => return Some("APK has no package name".to_owned()),
            _ => {}
        }
    }

    let versions = patch.compatible_versions();
    if !versions.is_empty() {
        match version {
            Some(ver) if !versions.iter().any(|v| v == ver) => {
                return Some(format!("incompatible version: {ver}"));
            }
            None => return Some("APK has no version name".to_owned()),
            _ => {}
        }
    }

    None
}
