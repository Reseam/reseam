// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! A patch's translations, assembled from the text of its name, description
//! and options.

use std::collections::BTreeMap;

use super::option_metadata::ReadOption;
use super::patch_text::Text;
use reseam_model::{OptionTranslation, PatchTranslation};

/// The patch's text by locale, ordered by tag so manifests are reproducible.
pub(in crate::bridge) fn translations(
    name: Option<&Text>,
    description: Option<&Text>,
    options: &[ReadOption],
) -> Vec<PatchTranslation> {
    let mut locales = BTreeMap::new();
    for (locale, text) in name.iter().flat_map(|it| &it.translations) {
        translation(&mut locales, locale).name = Some(text.clone());
    }
    for (locale, text) in description.iter().flat_map(|it| &it.translations) {
        translation(&mut locales, locale).description = Some(text.clone());
    }
    for option in options {
        let key = &option.declaration.key;
        for (locale, text) in &option.title.translations {
            option_translation(translation(&mut locales, locale), key).title = Some(text.clone());
        }
        for (locale, text) in option.description.iter().flat_map(|it| &it.translations) {
            option_translation(translation(&mut locales, locale), key).description =
                Some(text.clone());
        }
    }
    locales.into_values().collect()
}

fn translation<'a>(
    locales: &'a mut BTreeMap<String, PatchTranslation>,
    locale: &str,
) -> &'a mut PatchTranslation {
    locales
        .entry(locale.to_owned())
        .or_insert_with(|| PatchTranslation {
            locale: locale.to_owned(),
            name: None,
            description: None,
            options: Vec::new(),
        })
}

fn option_translation<'a>(
    translation: &'a mut PatchTranslation,
    key: &str,
) -> &'a mut OptionTranslation {
    let index = if let Some(index) = translation.options.iter().position(|it| it.key == key) {
        index
    } else {
        translation.options.push(OptionTranslation {
            key: key.to_owned(),
            title: None,
            description: None,
        });
        translation.options.len() - 1
    };
    &mut translation.options[index]
}
