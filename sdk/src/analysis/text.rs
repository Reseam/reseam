// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;

use reseam_dex::StringIdx;
use reseam_model::analysis::{
    CodeSite, DexString, RefSite, StringResource, TextMatches, XmlHit, XmlUse,
};

use super::Analysis;
use crate::error::Result;

/// A string resource while matches are gathered: its matching values by configuration.
struct Matched {
    value: String,
    qualifiers: Vec<String>,
}

impl Analysis {
    /// Where `text` comes from: string resources whose value contains it, in
    /// any configuration, the XML and code that use them, and DEX strings
    /// that contain it. Case is ignored, since apps show text in capitals
    /// through layout attributes and code.
    pub fn find_text(&mut self, text: &str) -> Result<TextMatches> {
        let text = Needle::new(text);
        let matched = self.matching_resources(&text)?;
        let mut resources = Vec::new();
        for (id, matched) in matched {
            resources.push(StringResource {
                resource: self.resource_id(id)?,
                value: matched.value,
                qualifiers: matched.qualifiers,
                loaded_by: sites(self.number_sites(i64::from(id as i32))?),
                xml: self.xml_hits(id)?,
            });
        }
        Ok(TextMatches {
            resources,
            dex_strings: self.dex_strings(&text)?,
        })
    }

    /// `string` resources containing `text`, by id; the value shown is the
    /// default configuration's when it matches, else the first that does.
    fn matching_resources(&self, text: &Needle) -> Result<BTreeMap<u32, Matched>> {
        let mut matched: BTreeMap<u32, Matched> = BTreeMap::new();
        self.for_each_string(|table, value, uses| {
            if !text.is_in(value) {
                return Ok(());
            }
            for used in uses {
                // File resources hold their path as a string too.
                let name = table
                    .resource_name(used.res_id)
                    .map_err(|error| self.opened.error(error))?;
                if name.is_none_or(|name| name.type_name != "string") {
                    continue;
                }
                let entry = matched.entry(used.res_id).or_insert_with(|| Matched {
                    value: value.to_owned(),
                    qualifiers: Vec::new(),
                });
                if used.qualifiers.is_empty() {
                    value.clone_into(&mut entry.value);
                }
                entry.qualifiers.push(used.qualifiers.clone());
            }
            Ok(())
        })?;
        for entry in matched.values_mut() {
            entry.qualifiers.sort();
            entry.qualifiers.dedup();
        }
        Ok(matched)
    }

    /// The XML elements that use resource `id`, with the code that loads
    /// their file and their view id.
    fn xml_hits(&mut self, id: u32) -> Result<Vec<XmlHit>> {
        let uses: Vec<(String, Option<u32>, String, Option<u32>)> = {
            let xml = self.xml_index()?;
            xml.uses(id)
                .map(|element| {
                    let file = &xml.files[element.file];
                    (
                        file.path.clone(),
                        file.resource,
                        element.tag.clone(),
                        element.view_id,
                    )
                })
                .collect()
        };
        let mut hits = Vec::new();
        for (file, resource, element, view_id) in uses {
            let loaded = |analysis: &Self, id: Option<u32>| -> Result<Vec<CodeSite>> {
                id.map_or(Ok(Vec::new()), |id| {
                    analysis.number_sites(i64::from(id as i32)).map(sites)
                })
            };
            hits.push(XmlHit {
                file_loaded_by: loaded(self, resource)?,
                view_loaded_by: loaded(self, view_id)?,
                usage: XmlUse {
                    file,
                    resource: resource.map(|id| self.resource_id(id)).transpose()?,
                    element,
                    view_id: view_id.map(|id| self.resource_id(id)).transpose()?,
                },
            });
        }
        Ok(hits)
    }

    /// DEX string constants containing `text`, with the code that loads each.
    fn dex_strings(&self, text: &Needle) -> Result<Vec<DexString>> {
        let mut by_value: BTreeMap<String, Vec<(usize, StringIdx)>> = BTreeMap::new();
        for (dex_idx, dex) in self.dex().iter().enumerate() {
            for (index, value) in dex.strings().iter().enumerate() {
                if text.is_in(&value) {
                    by_value
                        .entry(value.into_owned())
                        .or_default()
                        .push((dex_idx, StringIdx(index as u32)));
                }
            }
        }
        let mut found = Vec::new();
        for (value, strings) in by_value {
            let loaded_by = sites(self.strings_sites(&strings)?);
            if !loaded_by.is_empty() {
                found.push(DexString { value, loaded_by });
            }
        }
        Ok(found)
    }
}

fn sites(sites: Vec<RefSite>) -> Vec<CodeSite> {
    sites.into_iter().map(|site| site.site).collect()
}

/// Text to find, ignoring case.
struct Needle(String);

impl Needle {
    fn new(text: &str) -> Self {
        Self(text.to_lowercase())
    }

    fn is_in(&self, haystack: &str) -> bool {
        haystack.to_lowercase().contains(&self.0)
    }
}
