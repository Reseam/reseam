// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod inputs;
mod inspection;
mod output;

use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use reseam_apk::ApkFile;
use reseam_model::{ApkMetadata, PatchArtifact, PatchPhase, SigningKeyFiles};

use crate::ApkInspection;
use crate::metrics::{PatchProfiler, measure};
use crate::output::stage_unsigned;

pub(super) fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        writer
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

pub(super) fn apk(split: &str) -> Vec<u8> {
    let manifest = reseam_apk::axml::compile_xml(&format!(
        r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.test" android:versionCode="1" {split}><application android:label="Example" /></manifest>"#
    ), None).unwrap();
    zip(&[("AndroidManifest.xml", &manifest)])
}

pub(super) fn apk_metadata(path: &Path, splits: &[PathBuf]) -> crate::error::Result<ApkMetadata> {
    ApkInspection::open(path, splits)?.metadata()
}

/// Writes `apk` to `output` and signs it, as a run does after patching.
pub(super) fn publish(
    apk: ApkFile,
    output: &PatchArtifact,
    signing: Option<&SigningKeyFiles>,
) -> crate::error::Result<()> {
    let ((), opening) = measure(PatchPhase::OpenApk, || Ok::<_, crate::HostError>(()))?;
    let mut profiler = PatchProfiler::new(opening);
    stage_unsigned(apk, output, signing, &mut profiler)?.sign_and_publish(&mut profiler)
}
