// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io::Write;
use std::process::{Command, Stdio};

use reseam_model::{InstalledApp, InstalledSplit, MountRequest, PatchedSplit, RootScript};

use crate::mount::{
    install_script, installed_app, is_mounted, mount_script, original_script, query_script,
    status_script, unmount_script,
};

fn installed() -> InstalledApp {
    installed_app(
        "version_code=311\nversion_name=6.5\nfile=base.apk\nfile=split_config.en.apk\nfile=base.dm\n",
    )
    .unwrap()
    .unwrap()
}

fn request(version_code: i32, splits: &[&str]) -> MountRequest {
    MountRequest {
        package_name: "com.example.app".to_owned(),
        label: "Example's app".to_owned(),
        version_code,
        installed: installed(),
        base: "/data/local/tmp/base.apk".to_owned(),
        splits: splits
            .iter()
            .map(|&name| PatchedSplit {
                name: name.to_owned(),
                path: format!("/data/local/tmp/split_{name}.apk"),
            })
            .collect(),
        dex_metadata: Some("/data/local/tmp/base.dm".to_owned()),
    }
}

fn parses(script: &RootScript) {
    let mut shell = Command::new("sh")
        .arg("-n")
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    shell
        .stdin
        .take()
        .unwrap()
        .write_all(script.script.as_bytes())
        .unwrap();
    assert!(shell.wait().unwrap().success(), "{}", script.script);
}

#[test]
fn the_query_reads_the_installed_app_and_its_files() {
    assert_eq!(installed_app("\n").unwrap(), None);
    assert_eq!(
        installed(),
        InstalledApp {
            version_code: 311,
            version_name: "6.5".to_owned(),
            base_file: "base.apk".to_owned(),
            splits: vec![InstalledSplit {
                name: "config.en".to_owned(),
                file: "split_config.en.apk".to_owned(),
            }],
            dex_metadata_file: Some("base.dm".to_owned()),
        }
    );
}

#[test]
fn a_mount_needs_the_installed_version_and_splits() {
    for (version_code, splits, accepted) in [
        (311, &["config.en"][..], true),
        (312, &["config.en"][..], false),
        (311, &[][..], false),
        (311, &["config.en", "config.de"][..], false),
    ] {
        let script = mount_script(&request(version_code, splits));
        assert_eq!(script.is_ok(), accepted, "{version_code} {splits:?}");
        if let Ok(script) = script {
            assert!(
                script
                    .script
                    .contains("cp '/data/local/tmp/base.dm' \"$dir/apk/\"'base.dm'")
            );
            parses(&script);
        }
    }
}

#[test]
fn every_script_is_valid_shell_and_takes_only_package_names() {
    let package = "com.example.app";
    for script in [
        query_script(package),
        unmount_script(package),
        status_script(package),
        original_script(package, "split_config.en.apk"),
        install_script(package, &["/data/local/tmp/it's.apk".to_owned()]),
    ] {
        parses(&script.unwrap());
    }
    assert!(status_script("com.example; reboot").is_err());
    assert!(is_mounted("mounted\n") && !is_mounted("unmounted\n"));
}
