// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;
use std::path::Path;

use reseam_dex::{Loading, ParseOptions, Verification};
use reseam_storage::ScratchDir;
use tracing::{info, instrument};

use super::{ApkComponent, ApkFile, DexSet};
use crate::error::{Result, invalid};

impl ApkFile {
    #[instrument(level = "info", skip_all, fields(apk_path = %path.as_ref().display()))]
    pub fn open(path: impl AsRef<Path>, opts: ParseOptions) -> Result<Self> {
        Self::open_split(path, &[] as &[&Path], opts)
    }

    /// Opens an APK set given in any order: the component without a split
    /// identity is the base, and it must have a package name. Splits must have
    /// unique nonempty names and match its package and version code; invalid
    /// sets fail before any DEX is loaded.
    #[instrument(level = "info", skip_all)]
    pub fn open_split(
        base: impl AsRef<Path>,
        splits: &[impl AsRef<Path>],
        opts: ParseOptions,
    ) -> Result<Self> {
        let mut components: Vec<_> = std::iter::once(base.as_ref())
            .chain(splits.iter().map(AsRef::as_ref))
            .map(|path| ApkComponent::open(path, opts.classes))
            .collect::<Result<_>>()?;
        order_components(&mut components)?;
        Self::from_components(components, opts, None)
    }

    /// A session over components [`order_components`] put in order. It owns
    /// `scratch`, the directory a container's components were extracted to.
    pub(crate) fn from_components(
        mut components: Vec<ApkComponent>,
        opts: ParseOptions,
        scratch: Option<ScratchDir>,
    ) -> Result<Self> {
        if opts.classes == Loading::Eager {
            for component in &mut components {
                component.resources()?;
            }
        }
        let apk = Self {
            options: opts,
            dex: DexSet::load(&components, opts)?,
            components,
            _scratch: scratch,
            journal: reseam_journal::Journal::default(),
        };
        info!(
            package = apk.package_name().as_deref(),
            version = apk.version_name().as_deref(),
            components = apk.components.len(),
            dex_files = apk.dex.len(),
            "opened APK"
        );
        Ok(apk)
    }

    /// Patch-time inspection defers class and debug decoding and accepts stale
    /// DEX checksums and signatures, as repacked APKs commonly carry them.
    pub fn patch_options() -> ParseOptions {
        ParseOptions {
            classes: Loading::Deferred,
            debug_info: Loading::Deferred,
            annotations: Loading::Eager,
            checksum: Verification::Skip,
            signature: Verification::Skip,
            ..ParseOptions::default()
        }
    }
}

/// Puts the base first and checks the set: one base with a package name, and
/// splits with unique nonempty names that match its package and version code.
pub(crate) fn order_components(components: &mut [ApkComponent]) -> Result<()> {
    components.sort_by_key(|component| component.manifest().split_name().is_some());
    let base = components
        .first()
        .ok_or_else(|| invalid("apk set", "no base APK"))?
        .manifest();
    if base.split_name().is_some() {
        return Err(invalid(
            "apk set",
            "no base APK: every component is a split",
        ));
    }
    let package = base
        .package_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| invalid("apk set", "base APK has no package name"))?;
    let mut names = HashSet::new();
    for component in &components[1..] {
        let manifest = component.manifest();
        let name = manifest.split_name().ok_or_else(|| {
            invalid(
                "apk set",
                format!("multiple base APKs: {}", component.path().display()),
            )
        })?;
        if name.is_empty() || !names.insert(name.into_owned()) {
            return Err(invalid(
                "apk set",
                format!(
                    "empty or duplicate split name in {}",
                    component.path().display()
                ),
            ));
        }
        if manifest.package_name().as_deref() != Some(package.as_ref())
            || manifest.version_code() != base.version_code()
        {
            return Err(invalid(
                "apk set",
                format!(
                    "{} does not match the base APK package and version code",
                    component.path().display()
                ),
            ));
        }
    }
    Ok(())
}
