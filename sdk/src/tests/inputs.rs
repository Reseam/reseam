// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_apk::ScratchDir;
use reseam_model::{
    InstallMethod, PatchOutput, PatchRequest, PatchSelection, Problem, SdkError, Trust,
};

use super::{apk, apk_metadata};
use crate::{HostError, patch};
use reseam_fixtures::zip;

#[test]
fn an_unreadable_resource_table_is_an_unreadable_apk() {
    let tmp = ScratchDir::new("sdk-unreadable-resources").unwrap();
    let manifest = reseam_apk::axml::compile_xml(
        r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.test" android:versionCode="1"><application android:label="@0x7f010000" /></manifest>"#,
        None,
    )
    .unwrap();
    let broken = zip(&[
        ("AndroidManifest.xml", &manifest),
        (
            "resources.arsc",
            &[0x02, 0x00, 0x0c, 0x00, 0xff, 0xff, 0x00, 0x00],
        ),
    ]);
    for (name, bytes) in [
        ("broken.apk", broken.clone()),
        ("broken.apkm", zip(&[("base.apk", &broken)])),
    ] {
        let input = tmp.path().join(name);
        std::fs::write(&input, bytes).unwrap();
        let error = apk_metadata(&input, &[]).unwrap_err();
        assert_eq!(
            crate::error::classify(&error),
            Problem::unreadable_apk(&input),
            "{error:#}"
        );
    }
}

#[test]
fn incompatible_inputs_and_output_fail_before_loading_patches() {
    let tmp = ScratchDir::new("sdk-input-test").unwrap();
    let input = tmp.path().join("app.apkm");
    std::fs::write(
        &input,
        zip(&[
            ("base.apk", &apk("")),
            ("config.en.apk", &apk(r#"split="config.en""#)),
        ]),
    )
    .unwrap();
    let extra = tmp.path().join("extra.apk");
    std::fs::write(&extra, apk(r#"split="config.fr""#)).unwrap();
    assert!(apk_metadata(&input, &[extra]).is_err());
    let error = patch(
        &PatchRequest {
            apk_path: input.display().to_string(),
            split_paths: Vec::new(),
            bundle_paths: Vec::new(),
            trust: Trust::default(),
            selection: PatchSelection::default(),
            output: PatchOutput::SingleFile {
                path: tmp.path().join("out.apk").display().to_string(),
            },
            signing: None,
            dry_run: true,
            install_method: InstallMethod::Install,
            leave_out_failed: false,
        },
        |_| {},
    )
    .unwrap_err();
    assert_eq!(
        crate::error::classify(&error),
        Problem::SingleFileComponents { components: 2 },
        "{error:#}"
    );
    assert!(!tmp.path().join("out.apk").exists());
}

#[test]
fn host_failures_retain_problem_categories_and_sources() {
    use reseam_patcher::error::PatcherError;
    let older = || Problem::BundleTooOld {
        bundle: "example".into(),
        built: "0.3.0".into(),
        running: "0.15.0".into(),
    };
    let newer = || Problem::EngineTooOld {
        bundle: "example".into(),
        built: "1.0.0".into(),
        running: "0.15.0".into(),
    };
    for (error, expected) in [
        (
            HostError::Bundle {
                path: "patches.reseam".into(),
                source: PatcherError::Problem(older()),
            },
            older(),
        ),
        (HostError::Patcher(PatcherError::Problem(newer())), newer()),
        (
            HostError::Problem(Problem::SingleFileComponents { components: 2 }),
            Problem::SingleFileComponents { components: 2 },
        ),
        (
            HostError::InvalidRequest("invalid host request"),
            Problem::Other,
        ),
    ] {
        let failure = SdkError::from(error);
        assert_eq!(failure.problem, expected);
    }
}
