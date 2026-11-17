// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::PatchSpec;
use icu_locale::subtags::{Language, Region, Script, language};
use icu_locale::{Locale, LocaleCanonicalizer, LocaleExpander};
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
    /// as BCP 47 tags. Each field comes from the first translation in
    /// [`reading_order`] that has it, and a field none has keeps the source
    /// text.
    pub fn text(&self, locales: &[String]) -> PatchText {
        let chain = reading_order(&self.translations, locales);
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

/// Patch text is written in English, so English readers stop at the source.
const SOURCE_LANGUAGE: Language = language!("en");

/// The translations a reader of `locales` sees, in the order the app runtime
/// reads string tables in. For each locale, translations of its language in
/// the same script count, the locale's region first, then no region, then
/// other regions. An English locale ends the order, since the source is
/// English, after only a translation for its own region.
fn reading_order<'a>(
    translations: &'a [PatchTranslation],
    locales: &[String],
) -> Vec<&'a PatchTranslation> {
    let available: Vec<(Tag, &PatchTranslation)> = translations
        .iter()
        .filter_map(|it| Some((Tag::parse(&it.locale)?, it)))
        .collect();
    let mut order: Vec<&PatchTranslation> = Vec::new();
    for wanted in locales.iter().filter_map(|it| Tag::parse(it)) {
        let source = wanted.language == SOURCE_LANGUAGE;
        let mut matching: Vec<_> = available
            .iter()
            .filter_map(|(tag, it)| Some((tag.rank(&wanted)?, *it)))
            .filter(|(rank, _)| !source || *rank == Rank::Region)
            .collect();
        matching.sort_by_key(|(rank, _)| *rank);
        for (_, translation) in matching {
            if !order.iter().any(|it| std::ptr::eq(*it, translation)) {
                order.push(translation);
            }
        }
        if source {
            break;
        }
    }
    order
}

/// A locale as text matching compares it: canonical language, likely script
/// and the region it names.
struct Tag {
    language: Language,
    script: Option<Script>,
    region: Option<Region>,
}

/// How well a translation fits a wanted locale of the same language and script.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Rank {
    Region,
    NoRegion,
    OtherRegion,
}

impl Tag {
    fn parse(tag: &str) -> Option<Self> {
        let mut locale: Locale = tag.parse().ok()?;
        LocaleCanonicalizer::new_common().canonicalize(&mut locale);
        let region = locale.id.region;
        LocaleExpander::new_common().maximize(&mut locale.id);
        Some(Self {
            language: locale.id.language,
            script: locale.id.script,
            region,
        })
    }

    /// How this translation's tag fits `wanted`, or `None` for another
    /// language or script.
    fn rank(&self, wanted: &Tag) -> Option<Rank> {
        if self.language != wanted.language || self.script != wanted.script {
            return None;
        }
        Some(match self.region {
            None => Rank::NoRegion,
            region if region == wanted.region => Rank::Region,
            Some(_) => Rank::OtherRegion,
        })
    }
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
