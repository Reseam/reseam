// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io::{Cursor, Write};

use reseam_patcher::bundle::ENGINE_VERSION;
use zip::write::SimpleFileOptions;

use crate::authoring::{init_bundle, manifest_info};
use crate::error::HostError;

const MANIFEST: &str =
    "[bundle]\nname = \"example-patches\"\nauthor = \"Your Name\"\nformat_version = 1\n";
const SETTINGS: &str = "plugins {\n    id(\"app.reseam.workspace\") version \"0.1.0\"\n}\n\nrootProject.name = \"example-patches\"\n";

/// The template repository as the forge archives it: every file in one top-level folder.
fn template(settings: &str) -> Cursor<Vec<u8>> {
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let file = SimpleFileOptions::default();
    archive.add_directory("patches-template/", file).unwrap();
    for (name, text, mode) in [
        ("manifest.toml", MANIFEST, 0o644),
        ("settings.gradle.kts", settings, 0o644),
        ("gradlew", "#!/bin/sh\n", 0o755),
    ] {
        archive
            .start_file(
                format!("patches-template/{name}"),
                file.unix_permissions(mode),
            )
            .unwrap();
        archive.write_all(text.as_bytes()).unwrap();
    }
    Cursor::new(archive.finish().unwrap().into_inner())
}

#[test]
fn a_new_bundle_is_the_template_named_for_this_engine() {
    let parent = tempfile::tempdir().unwrap();
    let dir = parent.path().join("my-patches");
    init_bundle(template(SETTINGS), "my-patches", &dir).unwrap();

    let info = manifest_info(&dir.join("manifest.toml")).unwrap();
    assert_eq!(
        (info.name.as_str(), info.author.as_str()),
        ("my-patches", "Your Name")
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("settings.gradle.kts")).unwrap(),
        format!(
            "plugins {{\n    id(\"app.reseam.workspace\") version \"{ENGINE_VERSION}\"\n}}\n\nrootProject.name = \"my-patches\"\n"
        )
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(dir.join("gradlew"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755);
    }
}

/// The error of an init that fails, after checking it left nothing behind.
fn failed_init(name: &str, settings: &str) -> HostError {
    let parent = tempfile::tempdir().unwrap();
    let error = init_bundle(template(settings), name, &parent.path().join("bundle")).unwrap_err();
    assert_eq!(std::fs::read_dir(parent.path()).unwrap().count(), 0);
    error
}

#[test]
fn init_refuses_a_name_that_is_not_a_bundle_name() {
    assert!(matches!(
        failed_init("My Patches", SETTINGS),
        HostError::Patcher(_)
    ));
}

#[test]
fn init_fails_on_a_template_it_cannot_set_up() {
    let error = failed_init("my-patches", "plugins {}\n");
    assert!(
        matches!(error, HostError::TemplateSetting { setting, .. } if setting.starts_with("id("))
    );
}

#[test]
fn init_refuses_an_existing_folder() {
    let parent = tempfile::tempdir().unwrap();
    let error = init_bundle(template(SETTINGS), "my-patches", parent.path()).unwrap_err();
    assert!(matches!(error, HostError::Exists(path) if path == parent.path()));
}
