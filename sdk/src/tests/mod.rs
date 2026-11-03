// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod analysis;
mod changes;
mod inputs;
mod inspection;
mod mount;
mod output;
mod plan;
mod signing;
mod template;

use std::path::{Path, PathBuf};

use reseam_apk::ApkFile;
use reseam_model::{
    ApkMetadata, Compatibility, CompatiblePackage, PatchArtifact, PatchMetadata, PatchPhase,
    PatchRef, PatchSpec, SigningIdentity,
};

use crate::ApkInspection;
use crate::metrics::measure;
use crate::output::stage_unsigned;
use crate::progress::RunProgress;

pub(super) fn apk(split: &str) -> Vec<u8> {
    let manifest = reseam_apk::axml::compile_xml(&format!(
        r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.test" android:versionCode="1" {split}><application android:label="Example" /></manifest>"#
    ), None).unwrap();
    reseam_fixtures::apk(&manifest, &[])
}

pub(super) fn apk_metadata(path: &Path, splits: &[PathBuf]) -> crate::error::Result<ApkMetadata> {
    ApkInspection::open(path, splits)?.metadata()
}

/// Writes `apk` to `output` and signs it, as a run does after patching.
pub(super) fn publish(
    apk: ApkFile,
    output: &PatchArtifact,
    signing: Option<&SigningIdentity>,
) -> crate::error::Result<()> {
    let ((), opening) = measure(PatchPhase::OpenApk, || Ok::<_, crate::HostError>(()))?;
    let mut ignore = |_| {};
    let mut progress = RunProgress::new(opening, &mut ignore);
    stage_unsigned(apk, output, signing, &mut progress)?.sign_and_publish(&mut progress)
}

/// A patch of bundle `bundle`, enabled by default, for `com.example` at
/// `versions`, or for any app when `None`.
pub(super) fn spec(id: &str, versions: Option<&[&str]>) -> PatchSpec {
    PatchSpec {
        bundle: "bundle".to_owned(),
        id: id.to_owned(),
        name: id.to_owned(),
        hidden: false,
        description: String::new(),
        enabled_by_default: true,
        dependencies: Vec::new(),
        compatibility: match versions {
            None => Compatibility::Universal,
            Some(versions) => Compatibility::Packages {
                packages: vec![CompatiblePackage {
                    package: "com.example".to_owned(),
                    versions: versions.iter().map(|&v| v.to_owned()).collect(),
                    version_codes: Vec::new(),
                }],
            },
        },
        options: Vec::new(),
        rewrites_output: false,
    }
}

pub(super) fn reference(id: &str) -> PatchRef {
    PatchRef {
        bundle: "bundle".to_owned(),
        id: id.to_owned(),
    }
}

/// `spec` as an inspection lists it, with `fingerprint`.
pub(super) fn metadata(spec: PatchSpec, fingerprint: &str) -> PatchMetadata {
    PatchMetadata {
        spec,
        incompatibility: None,
        presets: Vec::new(),
        fingerprint: fingerprint.to_owned(),
    }
}
