// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_apk::ApkFile;
use reseam_patcher::bundle::{BUNDLE_FORMAT_VERSION, BundleArchive, BundleKey, pack};
use reseam_patcher::context::{ExtensionSet, PatchContext};
use reseam_patcher::engine::{self, PatchSelection, PatchStatus};
use reseam_patcher::{ApkVersion, Patch};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

pub static FIXTURE_JAR: OnceLock<PathBuf> = OnceLock::new();

pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

pub fn run_checked(cmd: &mut Command, context: &str) {
    let output = cmd
        .output()
        .unwrap_or_else(|e| panic!("{context}: failed to spawn: {e}"));
    assert!(
        output.status.success(),
        "{context} failed\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn build_fixture_jar() -> PathBuf {
    FIXTURE_JAR
        .get_or_init(|| {
            let root = workspace_root();
            let gradle = root.join("gradlew");
            let fixture_dir = root.join("tests/kotlin-runtime-bundle");

            run_checked(
                Command::new(&gradle)
                    .arg("-p")
                    .arg(&fixture_dir)
                    .arg("--no-daemon")
                    .arg("-Dorg.gradle.jvmargs=-Xmx1536m")
                    .arg("patchJar"),
                "build kotlin runtime test bundle",
            );

            fixture_dir.join("build/reseam/reseam-test-patches.jar")
        })
        .clone()
}

pub const TEST_SIGNING_SEED: [u8; 32] = [0x42; 32];

pub struct TestBundle {
    _dir: tempfile::TempDir,
    pub path: PathBuf,
}

pub fn write_bundle_reseam() -> TestBundle {
    let tmp = tempfile::tempdir().unwrap();
    let out_path = tmp.path().join("runtime-test-bundle.reseam");
    fs::copy(
        build_fixture_jar(),
        tmp.path().join("reseam-test-patches.jar"),
    )
    .unwrap();
    fs::write(
        tmp.path().join("manifest.toml"),
        format!(
            "[bundle]\nname = 'runtime-test-bundle'\nlicense = 'GPL-3.0'\nformat_version = {BUNDLE_FORMAT_VERSION}\n"
        ),
    )
    .unwrap();
    let key = tmp.path().join("bundle-signing.key");
    fs::write(&key, TEST_SIGNING_SEED).unwrap();
    pack(tmp.path(), None, &BundleKey::read(&key).unwrap(), &out_path).unwrap();

    TestBundle {
        _dir: tmp,
        path: out_path,
    }
}

pub fn apply_fixture(apk: &mut ApkFile, name: &str, id: &str) {
    let bundle = loaded_test_bundle();
    run_one_patch(
        &mut PatchContext::new(apk, ExtensionSet::app_runtime()),
        &bundle.patches().iter().collect::<Vec<_>>(),
        name,
        id,
    );
}

pub fn run_one_patch(ctx: &mut PatchContext<'_>, patches: &[&Patch], name: &str, id: &str) {
    let results = apply(
        ctx,
        patches,
        &PatchSelection {
            enable: [name.to_string()].into(),
            ..Default::default()
        },
    )
    .unwrap();
    let applied = results.iter().find(|result| {
        result.patch.to_string() == format!("runtime-test-bundle/app.reseam.test.{id}")
    });
    assert!(
        matches!(
            applied.map(|result| &result.status),
            Some(PatchStatus::Applied)
        ),
        "{name} did not apply: {:?}",
        results
            .iter()
            .map(|result| (&result.patch, &result.status))
            .collect::<Vec<_>>()
    );
}

pub fn loaded_test_bundle() -> reseam_patcher::bundle::PatchBundle {
    load_all(&write_bundle_reseam().path)
}

/// Every patch of the bundle at `path`, so one load serves selections a test picks later.
pub fn load_all(path: &Path) -> reseam_patcher::bundle::PatchBundle {
    let archive = BundleArchive::open(path).unwrap();
    let ids = archive
        .patches()
        .iter()
        .map(|spec| spec.reference.id.clone())
        .collect();
    archive.load(&ids).unwrap()
}

/// Plans `selection` for the APK in `ctx` and applies it, as a host run does.
pub fn apply(
    ctx: &mut PatchContext<'_>,
    patches: &[&Patch],
    selection: &PatchSelection,
) -> reseam_patcher::error::Result<Vec<engine::PatchResult>> {
    let specs: Vec<_> = patches.iter().map(|patch| patch.spec()).collect();
    let plan = engine::ResolvedPlan::resolve(
        &specs,
        selection,
        engine::Delivery::Install,
        ctx.apk().package_name().as_deref(),
        ApkVersion {
            name: ctx.apk().version_name().as_deref(),
            code: ctx.apk().version_code(),
        },
    )?;
    engine::apply_patches(ctx, &specs, plan, patches, |_| {}).map(engine::AppliedRun::into_results)
}
