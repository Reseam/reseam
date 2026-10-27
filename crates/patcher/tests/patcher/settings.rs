// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::apks::{SETTINGS_SCHEMA, entry, open_settings_test_apk};
use crate::common::bundles::{apply, apply_fixture, load_all, write_bundle_reseam};
use crate::common::code::{invoked_by, read_method, written_dex};
use reseam_patcher::Patch;
use reseam_patcher::context::{ExtensionSet, PatchContext};
use reseam_patcher::engine::{PatchSelection, PatchStatus};

const SETTINGS_STRINGS: &str = "assets/reseam/runtime-test-bundle/strings/TestStrings/default.json";

const RUNTIME_STRINGS: &str = "assets/reseam/runtime/strings/default.json";

#[test]
fn settings_pages_merge_selected_contributions_and_keep_nested_ancestors() {
    let bundle_file = write_bundle_reseam();
    let bundle = load_all(&bundle_file.path);
    let patches: Vec<&Patch> = bundle.patches().iter().collect();
    // Run twice against the same loaded host: pages from the first selection must not leak.
    for (patch, expected_pages, expected_keys) in [
        (
            "navigationSecondPatch",
            vec!["general", "media", "quality"],
            vec!["quality.first", "quality.second"],
        ),
        (
            "navigationFirstPatch",
            vec!["media", "quality"],
            vec!["quality.first"],
        ),
    ] {
        let (_dir, mut apk) = open_settings_test_apk();
        let selection = PatchSelection {
            enable: [format!("runtime-test-bundle/app.reseam.test.{patch}")].into(),
            ..Default::default()
        };
        let results = apply(
            &mut PatchContext::new(&mut apk, ExtensionSet::app_runtime()),
            &patches,
            &selection,
        )
        .unwrap();
        assert!(
            results
                .iter()
                .all(|r| !matches!(r.status, PatchStatus::Failed { .. }))
        );
        let bytes = entry(&mut apk, 0, SETTINGS_SCHEMA).unwrap();
        let schema: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let pages = schema["pages"].as_array().unwrap();
        assert_eq!(
            pages
                .iter()
                .map(|p| p["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            expected_pages
        );
        let child = pages.iter().find(|p| p["id"] == "quality").unwrap();
        assert_eq!(child["parent"], "media");
        assert_eq!(child["title"]["table"], "TestStrings");
        assert_eq!(child["title"]["name"], "page_quality");
        let table = entry(&mut apk, 0, SETTINGS_STRINGS).unwrap();
        let table: serde_json::Value = serde_json::from_slice(&table).unwrap();
        assert_eq!(table["page_quality"], "Quality");
        assert!(entry(&mut apk, 0, RUNTIME_STRINGS).is_some());
        let sections = schema["sections"].as_array().unwrap();
        assert!(sections[0]["page"].is_null());
        assert_eq!(sections[0]["settings"][0]["key"], "root.enabled");
        assert_eq!(sections[0]["settings"][1]["multiline"], true);
        let quality: Vec<_> = sections.iter().filter(|s| s["page"] == "quality").collect();
        assert_eq!(quality.len(), 1);
        assert_eq!(
            quality[0]["settings"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s["key"].as_str().unwrap())
                .collect::<Vec<_>>(),
            expected_keys
        );
        assert_eq!(quality[0]["settings"][0]["default"], true);
    }
}

#[test]
fn settings_schemas_carry_the_bundle_license_and_normalized_svg_icons() {
    let (_dir, mut apk) = open_settings_test_apk();
    apply_fixture(
        &mut apk,
        "Settings navigation first",
        "navigationFirstPatch",
    );
    let schema: serde_json::Value =
        serde_json::from_slice(&entry(&mut apk, 0, SETTINGS_SCHEMA).unwrap()).unwrap();
    assert_eq!(schema["license"], "GPL-3.0");
    let about = &schema["about"];
    assert_eq!(about["links"][0]["label"]["text"], "Forum");
    assert_eq!(about["links"][0]["url"], "https://example.com/forum");
    assert_eq!(
        about["links"][0]["icon"]["paths"][0]["data"],
        "M 9 18 L 15 12 L 9 6"
    );
    assert_eq!(about["actions"][0]["label"]["name"], "page_media");
    assert_eq!(about["actions"][0]["url"], "https://example.com/donate");
    let media = schema["pages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|page| page["id"] == "media")
        .unwrap();
    let chevron = &media["icon"]["paths"][0];
    assert_eq!(chevron["data"], "M 9 18 L 15 12 L 9 6");
    assert!(
        chevron["fill"].is_null(),
        "fill=\"none\" on <svg> is inherited"
    );
    assert!(
        chevron["stroke"]["paint"].is_null(),
        "currentColor takes the tint"
    );
    assert_eq!(chevron["stroke"]["cap"], "round");
    let dot = &schema["sections"][0]["settings"][0]["icon"];
    assert_eq!(dot["width"], 4.0);
    let data = dot["paths"][0]["data"].as_str().unwrap();
    assert!(
        data.starts_with("M 0 2 C ") && data.ends_with(" Z"),
        "the circle becomes cubics, shifted to the viewBox origin: {data}"
    );
    assert_eq!(dot["paths"][0]["fill"]["paint"], "#FFFF0000");
}

#[test]
fn settings_hosts_start_the_app_runtime_and_gate_on_the_bundles_own_settings() {
    use reseam_dex::Instruction::ConstString;

    let (_dir, mut apk) = open_settings_test_apk();
    apply_fixture(&mut apk, "Settings gated entry", "gatedEntry");
    let written = written_dex(&apk, 0);
    assert_eq!(
        invoked_by(&written, "Lapp/reseam/AppEntry;", "onCreate"),
        [
            "Lapp/reseam/runtime/Reseam;->init",
            "Lapp/reseam/runtime/Reseam;->isEnabled",
            "Lcom/example/Observer;->started",
        ],
        "the app runtime starts before any entry code reads a setting"
    );
    let code = read_method(&written, "Lapp/reseam/AppEntry;", "onCreate")
        .unwrap()
        .code
        .unwrap();
    assert!(
        code.instructions().iter().any(|instruction| matches!(
            instruction,
            ConstString { string, .. } if written.strings().get(*string) == "runtime-test-bundle"
        )),
        "gates name the bundle whose settings they read"
    );
    assert!(
        apk.dex().iter().any(|dex| dex
            .find_class_index("Lapp/reseam/runtime/Reseam;")
            .is_some()),
        "the engine links its app runtime"
    );
}
