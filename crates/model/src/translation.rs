// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::PatchSpec;
use serde::{Deserialize, Serialize};

/// A patch's text in one locale. Fields the locale leaves untranslated are
/// absent and fall back as [`PatchSpec::text`] describes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct PatchTranslation {
    /// A BCP 47 tag, such as `de` or `pt-BR`.
    pub locale: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[boltffi::default(None)]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[boltffi::default(None)]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<OptionTranslation>,
}

/// An option's text in the locale of the [`PatchTranslation`] that holds it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct OptionTranslation {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[boltffi::default(None)]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[boltffi::default(None)]
    pub description: Option<String>,
}

/// What users see of a patch, in the locale [`PatchSpec::text`] picked for each field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct PatchText {
    pub name: String,
    pub description: String,
    /// In the order of [`PatchSpec::options`].
    pub options: Vec<OptionText>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[boltffi::data]
pub struct OptionText {
    pub key: String,
    pub title: String,
    pub description: String,
}

impl PatchSpec {
    /// The patch's text for a user who reads `locales`, most preferred first,
    /// as BCP 47 tags. Each field comes from the first tag that translates it,
    /// trying every tag as itself, then its language and script, then its
    /// language, the order the app runtime reads string tables in. A field no
    /// tag translates keeps the source text.
    pub fn text(&self, locales: &[String]) -> PatchText {
        let chain: Vec<&PatchTranslation> = fallbacks(locales)
            .iter()
            .filter_map(|tag| self.translations.iter().find(|it| it.locale == *tag))
            .collect();
        let options = self.options.iter().map(|declared| {
            let translated = || {
                chain
                    .iter()
                    .flat_map(|it| &it.options)
                    .filter(|it| it.key == declared.key)
            };
            OptionText {
                key: declared.key.clone(),
                title: translated()
                    .find_map(|it| it.title.as_deref())
                    .unwrap_or(&declared.title)
                    .to_owned(),
                description: translated()
                    .find_map(|it| it.description.as_deref())
                    .unwrap_or(&declared.description)
                    .to_owned(),
            }
        });
        PatchText {
            name: chain
                .iter()
                .find_map(|it| it.name.as_deref())
                .unwrap_or(&self.name)
                .to_owned(),
            description: chain
                .iter()
                .find_map(|it| it.description.as_deref())
                .unwrap_or(&self.description)
                .to_owned(),
            options: options.collect(),
        }
    }
}

fn fallbacks(locales: &[String]) -> Vec<String> {
    let mut tags = Vec::new();
    for locale in locales {
        let mut parts = locale.split('-');
        let language = parts.next().unwrap_or_default();
        let script = parts
            .next()
            .filter(|it| it.len() == 4 && it.chars().all(|c| c.is_ascii_alphabetic()));
        let candidates = [
            Some(locale.clone()),
            script.map(|script| format!("{language}-{script}")),
            Some(language.to_owned()),
        ];
        for tag in candidates.into_iter().flatten() {
            if !tag.is_empty() && !tags.contains(&tag) {
                tags.push(tag);
            }
        }
    }
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shared with the browser package, whose `patchText` must resolve the same way.
    #[derive(Deserialize)]
    struct Cases {
        spec: PatchSpec,
        cases: Vec<Case>,
    }

    #[derive(Deserialize)]
    struct Case {
        locales: Vec<String>,
        name: String,
        title: String,
        description: String,
    }

    #[test]
    fn each_field_comes_from_the_first_tag_that_translates_it() {
        let Cases { spec, cases } =
            serde_json::from_str(include_str!("../tests/data/patch_text.json")).unwrap();
        for case in cases {
            let text = spec.text(&case.locales);
            assert_eq!(
                (text.name, text.options[0].title.clone(), text.description),
                (case.name, case.title, case.description),
                "{:?}",
                case.locales
            );
        }
    }
}
