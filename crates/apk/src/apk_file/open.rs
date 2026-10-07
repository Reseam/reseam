// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;
use std::path::Path;

use reseam_dex::{MultiDexContainer, ParseOptions};
use tracing::{info, instrument};

use super::{ApkComponent, ApkFile, ComponentIndex, DexOrigin, DexSource};
use crate::dex;
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
        components.sort_by_key(|component| component.manifest().split_name().is_some());
        Self::from_components(components, opts)
    }

    pub(crate) fn from_components(
        mut components: Vec<ApkComponent>,
        opts: ParseOptions,
    ) -> Result<Self> {
        validate_components(&components)?;
        let mut dex = MultiDexContainer::new();
        let mut dex_origins = Vec::new();
        for (index, component) in components.iter_mut().enumerate() {
            if opts.classes == reseam_dex::types::header::Loading::Eager {
                component.resources()?;
            }
            for (name, file) in dex::load_dex(component.archive(), opts)? {
                dex.add_dex(file);
                dex_origins.push(DexOrigin {
                    component: ComponentIndex(index),
                    name,
                    kind: DexSource::Archive,
                });
            }
        }
        let apk = Self {
            options: opts,
            components,
            dex,
            dex_origins,
            scratch: None,
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
        use reseam_dex::types::header::{Loading, Verification};
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

pub(crate) fn validate_components(components: &[ApkComponent]) -> Result<()> {
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
