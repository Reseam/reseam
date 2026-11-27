// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
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
    for (patch, expected_pages, expected_keys, first_default) in [
        (
            "navigationSecondPatch",
            vec!["general", "media", "quality"],
            vec!["quality.first", "quality.second"],
            false,
        ),
        (
            "navigationFirstPatch",
            vec!["media", "quality"],
            vec!["quality.first"],
            true,
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
        assert_eq!(sections[0]["settings"][0]["restart"], true);
        assert_eq!(sections[0]["settings"][1]["restart"], false);
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
        assert_eq!(quality[0]["settings"][0]["default"], first_default);
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
    assert_eq!(
        about["links"][1]["icon"], about["links"][0]["icon"],
        "Lucide's chevron-right draws as its single-icon SVG does"
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
fn settings_screens_show_only_the_icons_a_patch_lists() {
    let (_dir, mut apk) = open_settings_test_apk();
    apply_fixture(&mut apk, "Settings icon keys", "settingsIconKeys");
    let schema: serde_json::Value =
        serde_json::from_slice(&entry(&mut apk, 0, SETTINGS_SCHEMA).unwrap()).unwrap();
    let media = schema["pages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|page| page["id"] == "media")
        .unwrap();
    assert!(media["icon"].is_object());
    assert_eq!(schema["sections"][0]["settings"][0]["key"], "root.enabled");
    assert!(schema["sections"][0]["settings"][0]["icon"].is_null());
}

#[test]
fn settings_rows_carry_their_dependency_and_the_code_patches_give_them() {
    let (_dir, mut apk) = open_settings_test_apk();
    apply_fixture(&mut apk, "Settings row behaviour", "settingsRowBehaviour");
    let schema: serde_json::Value =
        serde_json::from_slice(&entry(&mut apk, 0, SETTINGS_SCHEMA).unwrap()).unwrap();
    let settings = schema["sections"][0]["settings"].as_array().unwrap();
    let toggle = settings
        .iter()
        .find(|s| s["key"] == "root.enabled")
        .unwrap();
    let text = settings.iter().find(|s| s["key"] == "root.lines").unwrap();
    assert!(toggle["enabledBy"].is_null());
    assert_eq!(toggle["code"], serde_json::json!(["change", "enabled"]));
    assert_eq!(text["enabledBy"], "root.enabled");
    assert_eq!(text["code"], serde_json::json!(["summary", "validate"]));
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

#[test]
fn settings_actions_list_a_row_and_run_the_patch_code_when_tapped() {
    use reseam_dex::Instruction::ConstString;

    let (_dir, mut apk) = open_settings_test_apk();
    apply_fixture(&mut apk, "Settings action", "actionRow");
    let schema: serde_json::Value =
        serde_json::from_slice(&entry(&mut apk, 0, SETTINGS_SCHEMA).unwrap()).unwrap();
    let action = schema["sections"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|section| section["settings"].as_array().unwrap())
        .find(|setting| setting["key"] == "share")
        .unwrap();
    assert_eq!(action["type"], "action");
    assert_eq!(action["title"]["name"], "share");
    assert_eq!(action["summary"]["name"], "share_summary");
    assert!(action.get("default").is_none(), "an action stores nothing");

    let written = (0..apk.dex().len())
        .map(|index| written_dex(&apk, index))
        .find(|dex| read_method(dex, "Lapp/reseam/runtime/Reseam;", "runAction").is_some())
        .unwrap();
    assert_eq!(
        invoked_by(&written, "Lapp/reseam/runtime/Reseam;", "runAction"),
        [
            "Ljava/lang/String;->equals",
            "Ljava/lang/String;->equals",
            "Lcom/example/Observer;->shared",
        ],
        "the action's code runs only for its own bundle and key"
    );
    let code = read_method(&written, "Lapp/reseam/runtime/Reseam;", "runAction")
        .unwrap()
        .code
        .unwrap();
    let strings: Vec<String> = code
        .instructions()
        .iter()
        .filter_map(|instruction| match instruction {
            ConstString { string, .. } => Some(written.strings().get(*string).into_owned()),
            _ => None,
        })
        .collect();
    assert_eq!(strings, ["runtime-test-bundle", "share"]);
}

#[test]
fn an_action_without_code_fails_the_settings_host() {
    let bundle_file = write_bundle_reseam();
    let bundle = load_all(&bundle_file.path);
    let patches: Vec<&Patch> = bundle.patches().iter().collect();
    let (_dir, mut apk) = open_settings_test_apk();
    let results = apply(
        &mut PatchContext::new(&mut apk, ExtensionSet::app_runtime()),
        &patches,
        &PatchSelection {
            enable: ["runtime-test-bundle/app.reseam.test.uncodedActionRow".to_owned()].into(),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        results.iter().any(|result| matches!(
            &result.status,
            PatchStatus::Failed { reason } if reason.contains("share") && reason.contains("onAction")
        )),
        "{results:?}"
    );
}

#[test]
fn a_settings_layout_places_rows_keeps_stored_values_and_describes_row_kinds() {
    let (_dir, mut apk) = open_settings_test_apk();
    apply_fixture(&mut apk, "Settings layout", "settingsLayout");
    let schema: serde_json::Value =
        serde_json::from_slice(&entry(&mut apk, 0, SETTINGS_SCHEMA).unwrap()).unwrap();
    let sections: Vec<(Option<&str>, Vec<&str>)> = schema["sections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|section| {
            (
                section["title"]["name"].as_str(),
                section["settings"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|setting| setting["key"].as_str().unwrap())
                    .collect(),
            )
        })
        .collect();
    assert_eq!(
        sections,
        [
            (None, vec!["layout.second", "layout.note"]),
            (Some("section_general"), vec!["layout.first"]),
            (None, vec!["layout.first"]),
            (
                Some("section_quality"),
                vec!["layout.level", "layout.tint", "layout.range"]
            ),
            (None, vec!["layout.run"]),
        ],
        "the layout places its rows first and in its order, a row on each page it lists; the \
         rest keep their section"
    );
    let stored: Vec<&str> = schema["stored"]
        .as_array()
        .unwrap()
        .iter()
        .map(|setting| setting["key"].as_str().unwrap())
        .collect();
    assert_eq!(stored, ["layout.stored", "layout.hidden"]);

    let setting = |key: &str| {
        schema["sections"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|section| section["settings"].as_array().unwrap())
            .find(|setting| setting["key"] == key)
            .unwrap()
            .clone()
    };
    let typed: Vec<(&str, serde_json::Value)> = ["layout.level", "layout.tint", "layout.range"]
        .into_iter()
        .map(|key| {
            let setting = setting(key);
            (
                key,
                serde_json::json!([setting["type"], setting["default"]]),
            )
        })
        .collect();
    assert_eq!(
        typed,
        [
            ("layout.level", serde_json::json!(["number", 5.0])),
            ("layout.tint", serde_json::json!(["color", 0xFFFF_0000_u32])),
            (
                "layout.range",
                serde_json::json!(["range", {"start": 0.2, "end": 0.8}])
            ),
        ],
        "numbers, colors and ranges reach the app typed, a color as ARGB"
    );
    let run = setting("layout.run");
    assert_eq!(run["type"], "note");
    assert_eq!(run["title"]["name"], "page_layout");
    let page = schema["pages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|page| page["id"] == "layout.run")
        .unwrap();
    assert_eq!(page["after"], "layout.first");
    assert_eq!(page["heading"]["name"], "section_general");
    let note = setting("layout.note");
    assert_eq!(note["type"], "note");
    assert_eq!(note["summary"]["name"], "layout_note_summary");
    assert!(note.get("default").is_none(), "a note stores nothing");
}
