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

use std::fmt::Write;
use std::path::{Path, PathBuf};

use ed25519_dalek::{Signer, SigningKey};
use reseam_apk::ApkFile;
use reseam_fixtures::zip;
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

/// The manifest of bundle `example`: `patches` holds its `[[patches]]` tables,
/// `fingerprints` their fingerprints by id.
pub(super) fn catalog_manifest(fingerprints: &[(&str, &str)], patches: &str) -> String {
    let fingerprints = fingerprints
        .iter()
        .fold(String::new(), |mut toml, (id, fingerprint)| {
            writeln!(toml, "\"{id}\" = \"{fingerprint}\"").unwrap();
            toml
        });
    format!(
        r#"
[bundle]
name = "example"
format_version = 1
engine = "{}"

[files]
"patches.jar" = "{}"

[fingerprints]
{fingerprints}{patches}"#,
        reseam_patcher::bundle::ENGINE_VERSION,
        "00".repeat(32),
    )
}

/// Writes a bundle holding `manifest` to `path`, with `key`'s signature over
/// `signed`.
pub(super) fn write_catalog(path: &Path, manifest: &str, signed: &str, key: &SigningKey) {
    std::fs::write(
        path,
        zip(&[
            (
                "mimetype",
                reseam_patcher::bundle::BUNDLE_MIMETYPE.as_bytes(),
            ),
            ("manifest.toml", manifest.as_bytes()),
            ("manifest.pubkey", &key.verifying_key().to_bytes()),
            ("manifest.sig", &key.sign(signed.as_bytes()).to_bytes()),
            // This payload cannot be loaded, regardless of the client's trust.
            ("patches.jar", b"not executable code"),
        ]),
    )
    .unwrap();
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
