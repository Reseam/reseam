// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::bundles::write_bundle_reseam;
use reseam_patcher::bundle::BundleArchive;

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
    let universal = text("universal-marker");
    assert_eq!(universal.name, "universal-marker");
    assert_eq!(
        universal.description,
        "Nennt kein Paket, passt also zu jeder App und wartet, bis sie gewählt wird"
    );
}
