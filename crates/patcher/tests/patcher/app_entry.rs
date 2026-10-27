// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::apks::{
    entry, manifest_with_application, open_dex_apk, open_split_test_apk_with,
};
use crate::common::bundles::{apply, apply_fixture, loaded_test_bundle};
use crate::common::code::{dex_bytes, invoked_by, read_method, written_dex};
use crate::common::dex;
use reseam_dex::{DexFile, DexHeader, DexVersion};
use reseam_patcher::Patch;
use reseam_patcher::context::{ExtensionSet, PatchContext};
use reseam_patcher::engine::{PatchSelection, PatchStatus};

const APP_ENTRY_HOOK: &str = "Lapp/reseam/AppEntry;->onCreate";

#[test]
fn app_entry_unseals_the_final_on_create_it_overrides() {
    use reseam_dex::AccessFlags;

    const BASE: &str = "Lcom/example/AppBase;";
    const APP: &str = "Lcom/example/App;";

    let (_apk_dir, mut apk) = open_split_test_apk_with(
        &manifest_with_application("com.example.App"),
        &[dex!("entry_app"), dex!("entry_base")],
    );
    apply_fixture(&mut apk, "app-entry-hook", "appEntryHook");

    assert_eq!(
        invoked_by(&written_dex(&apk, 1), APP, "onCreate"),
        [APP_ENTRY_HOOK, "Lcom/example/AppBase;->onCreate"],
        "the added onCreate must call the hook, then the inherited onCreate"
    );
    assert_eq!(
        invoked_by(&written_dex(&apk, 0), "Lapp/reseam/AppEntry;", "onCreate"),
        ["Lcom/example/Observer;->started"]
    );
    assert!(
        !method_flags(&written_dex(&apk, 0), BASE, "onCreate")
            .unwrap()
            .contains(AccessFlags::FINAL),
        "the inherited onCreate is still final, so the override is a LinkageError"
    );
}

/// Protected manifests scramble attribute name strings; the platform reads `android:` attributes by
/// resource ID, so the SDK must find, read and replace them the same way.
#[test]
fn app_entry_follows_an_application_swapped_after_the_hook_was_added() {
    let manifest = manifest_with_application("com.example.WrapperApp");
    let at = manifest
        .windows(4)
        .position(|window| window == b"name")
        .unwrap();
    let scrambled = [&manifest[..at], b"zzzz", &manifest[at + 4..]].concat();
    for manifest in [manifest, scrambled] {
        assert_app_entry_follows_swapped_application(&manifest);
    }
}

fn assert_app_entry_follows_swapped_application(manifest: &[u8]) {
    const REAL: &str = "Lcom/example/RealApp;";
    const WRAPPER: &str = "Lcom/example/WrapperApp;";

    let (_apk_dir, mut apk) = open_split_test_apk_with(manifest, &[dex!("wrapped_application")]);
    apply_fixture(&mut apk, "unwrap-application", "unwrapApplication");
    let written =
        reseam_apk::AxmlDocument::parse(&entry(&mut apk, 0, "AndroidManifest.xml").unwrap())
            .unwrap();
    let application = written.find_element("application").unwrap();
    assert_eq!(
        written.attributes(application).len(),
        1,
        "the swap must replace android:name, not add a second one"
    );
    assert_eq!(
        written.application_name().as_deref(),
        Some("com.example.RealApp")
    );

    let written = written_dex(&apk, 0);
    assert_eq!(
        invoked_by(&written, REAL, "onCreate"),
        [APP_ENTRY_HOOK, "Landroid/app/Application;->onCreate"],
        "the hook must run from the Application the final manifest names"
    );
    assert!(
        method_flags(&written, WRAPPER, "onCreate").is_none(),
        "the wrapper the app no longer starts must not receive the hook"
    );
}

#[test]
fn app_entry_fails_the_patch_when_the_manifest_names_no_application() {
    let (_apk_dir, mut apk) =
        open_dex_apk(&[&dex_bytes(&DexFile::new(DexHeader::new(DexVersion::V035)))]);
    let bundle = loaded_test_bundle();
    let patches: Vec<&Patch> = bundle.patches().iter().collect();
    let results = apply(
        &mut PatchContext::new(&mut apk, ExtensionSet::app_runtime()),
        &patches,
        &PatchSelection {
            enable: ["app-entry-hook".to_string()].into(),
            ..Default::default()
        },
    )
    .unwrap();
    let status = &results
        .iter()
        .find(|result| result.patch.id.ends_with(".appEntryHook"))
        .unwrap()
        .status;
    assert!(matches!(status, PatchStatus::Failed { .. }), "{status:?}");
}

fn method_flags(dex: &DexFile, owner: &str, name: &str) -> Option<reseam_dex::AccessFlags> {
    read_method(dex, owner, name).map(|method| method.access_flags)
}
