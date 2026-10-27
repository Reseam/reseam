// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::apks::{manifest_bytes, open_dex_apk, open_split_test_apk_with};
use crate::common::bundles::{loaded_test_bundle, run_one_patch};
use crate::common::code::dex_bytes;
use crate::common::dex;
use reseam_dex::{DexFile, DexHeader, DexVersion};
use reseam_patcher::Patch;
use reseam_patcher::context::{ExtensionSet, PatchContext};
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn queries_skip_the_classes_a_bundle_extension_defines() {
    use reseam_patcher::context::ExtensionSet;

    const APP_STATE: &str = "Lcom/example/VideoState;";
    const EXT_STATE: &str = "Lapp/reseam/test/ext/VideoState;";

    let extension_dir = tempfile::tempdir().unwrap();
    let extension_path = extension_dir.path().join("ext.dex");
    fs::write(&extension_path, dex!("extension_shadow")).unwrap();

    let (_apk_dir, mut apk) =
        open_split_test_apk_with(&manifest_bytes("1.0-base", None), &[dex!("query_domain")]);
    let bundle = loaded_test_bundle();
    let patches: Vec<&Patch> = bundle.patches().iter().collect();
    let mut ctx = PatchContext::new(&mut apk, ExtensionSet::load(&[extension_path]).unwrap());
    run_one_patch(&mut ctx, &patches, "extension-shadow", "extensionShadow");

    assert_eq!(
        apk.read_component_entry(0, "assets/extension-shadow.txt")
            .unwrap(),
        Some(format!("{APP_STATE}|{EXT_STATE}").into_bytes()),
        "a plain query must find the app's class; includeExtensions() widens it to the bundle's"
    );
}

const SHARED_CLASS: &str = "Lapp/reseam/test/Shared;";

fn extension_files(dir: &Path, files: &[(&str, &[u8])]) -> Vec<PathBuf> {
    files
        .iter()
        .map(|(name, bytes)| {
            let path = dir.join(format!("{name}.dex"));
            fs::write(&path, bytes).unwrap();
            path
        })
        .collect()
}

#[test]
fn extensions_share_a_class_they_define_identically() {
    use reseam_patcher::context::ExtensionSet;

    let dir = tempfile::tempdir().unwrap();
    let paths = extension_files(
        dir.path(),
        &[("a", dex!("shared_class_a")), ("b", dex!("shared_class_b"))],
    );
    let (_apk_dir, mut apk) = open_dex_apk(&[dex!("types")]);
    let mut ctx = PatchContext::new(&mut apk, ExtensionSet::load(&paths).unwrap());
    ctx.find_or_link_class("Lapp/reseam/test/Bee;").unwrap();
    ctx.find_or_link_class("Lapp/reseam/test/A;").unwrap();
    drop(ctx);

    let definitions = apk
        .dex()
        .iter()
        .filter(|dex| dex.find_class_index(SHARED_CLASS).is_some())
        .count();
    assert_eq!(definitions, 1, "both extensions must link the one copy");
}

#[test]
fn bundles_cannot_define_app_runtime_classes() {
    use reseam_dex::AccessFlags;
    use reseam_patcher::context::ExtensionSet;

    let mut dex = DexFile::new(DexHeader::new(DexVersion::V035));
    dex.create_class(
        "Lapp/reseam/runtime/Reseam;",
        AccessFlags::PUBLIC,
        Some("Ljava/lang/Object;"),
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let paths = extension_files(dir.path(), &[("runtime", &dex_bytes(&dex))]);
    let Err(error) = ExtensionSet::load(&paths) else {
        panic!("a bundle's copy of the app runtime must not load");
    };
    assert!(
        error.to_string().contains("Lapp/reseam/runtime/Reseam;"),
        "{error}"
    );
}

#[test]
fn extensions_defining_a_class_differently_conflict() {
    use reseam_patcher::error::{ExtensionError, PatcherError};

    let dir = tempfile::tempdir().unwrap();
    let paths = extension_files(
        dir.path(),
        &[
            ("a", dex!("shared_class_a")),
            ("conflict", dex!("shared_class_conflict")),
        ],
    );
    let Err(error) = ExtensionSet::load(&paths) else {
        panic!("differing definitions of {SHARED_CLASS} must not load");
    };
    assert!(
        matches!(
            &error,
            PatcherError::Extension {
                reason: ExtensionError::Conflict { descriptor, .. },
                ..
            } if descriptor == SHARED_CLASS
        ),
        "{error}"
    );
}
