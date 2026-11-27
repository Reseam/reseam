// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::bundles::write_bundle_reseam;
use reseam_patcher::bundle::BundleArchive;
use reseam_patcher::options::OptionValue;

#[test]
fn the_signed_catalog_carries_the_translations_of_patch_text() {
    let bundle = write_bundle_reseam();
    let archive = BundleArchive::open(&bundle.path).unwrap();
    let german = ["de-AT".to_owned()];
    let text = |name: &str| {
        archive
            .patches()
            .iter()
            .find(|spec| spec.name == name)
            .unwrap()
            .text(&german)
    };

    let hide_ads = text("Hide Ads");
    assert_eq!(hide_ads.name, "Werbung ausblenden");
    assert_eq!(hide_ads.options[0].title, "Markierung");
    let labels: Vec<&str> = hide_ads.options[0]
        .presets
        .iter()
        .map(|preset| preset.label.as_str())
        .collect();
    assert_eq!(labels, ["Erste", "Second"]);
    let universal = text("universal-marker");
    assert_eq!(universal.name, "universal-marker");
    assert_eq!(
        universal.description,
        "Nennt kein Paket, passt also zu jeder App und wartet, bis sie gewählt wird"
    );
}

#[test]
fn presets_carry_typed_values_and_strict_ones_reject_other_values() {
    let bundle = write_bundle_reseam();
    let archive = BundleArchive::open(&bundle.path).unwrap();
    let spec = archive
        .patches()
        .iter()
        .find(|spec| spec.name == "Hide Ads")
        .unwrap();
    let option = |key: &str| spec.options.iter().find(|it| it.key == key).unwrap();
    let marker = option("marker");
    assert!(marker.allows_custom);
    assert_eq!(marker.valid_values, None);
    assert_eq!(marker.check(&OptionValue::Text("custom".into())), Ok(()));
    let level = option("level");
    assert!(!level.allows_custom);
    let values: Vec<&OptionValue> = level.presets.iter().map(|preset| &preset.value).collect();
    assert_eq!(values, [&OptionValue::Int(1), &OptionValue::Int(2)]);
    assert!(level.check(&OptionValue::Int(3)).is_err());
}
