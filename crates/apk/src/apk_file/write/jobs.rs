// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{HashMap, HashSet};

use super::super::component::{ApkComponent, EntryEdit};
use super::super::dex_set::{DexIndex, DexMember, DexSource};
use super::super::{ApkFile, ComponentIndex};
use crate::entry::{EntryName, dex_entry_names, dex_name};
use crate::error::Result;
use crate::zip::Compression;

/// One DEX entry to serialize, stored the way its component stores DEX.
pub(in crate::apk_file) struct DexJob {
    pub members: Vec<DexMember>,
    pub component: ComponentIndex,
    /// The entry the session holds it as; `None` for a part split off a DEX
    /// too large for one entry.
    pub source: Option<EntryName>,
    /// Its name in the output.
    pub name: EntryName,
    pub compression: Compression,
}

/// The names a component's DEX entries are written as. Android loads
/// `classes.dex`, `classes2.dex` and on until the first missing ordinal, so
/// the entries the component holds from its archive or as injected files are
/// numbered in order without gaps, and deleting one renames those after it.
/// DEX the session added, and parts split off to fit, take the ordinals after
/// them.
pub(in crate::apk_file) struct DexNames {
    /// Output names of the held entries, by the names the session holds them as.
    held: HashMap<EntryName, EntryName>,
    last_ordinal: u32,
}

impl DexNames {
    fn of(component: &ApkComponent) -> Self {
        let held: Vec<EntryName> = component
            .reserved_dex_names()
            .filter(|name| {
                component.contains(name)
                    && !matches!(component.edits().get(*name), Some(EntryEdit::Dex))
            })
            .map(EntryName::from)
            .collect();
        let mut held = dex_entry_names(&held);
        held.dedup();
        let mut names = Self {
            held: HashMap::new(),
            last_ordinal: 0,
        };
        for name in held {
            let output = names.next();
            names.held.insert(name, output);
        }
        names
    }

    /// The output name of entry `name`; entries that are not DEX keep theirs.
    pub(super) fn output<'a>(&'a self, name: &'a str) -> &'a str {
        self.held.get(name).map_or(name, EntryName::as_str)
    }

    /// The name of the next free ordinal, for a DEX entry the component did
    /// not hold.
    fn next(&mut self) -> EntryName {
        self.last_ordinal += 1;
        dex_name(self.last_ordinal)
    }
}

impl ApkFile {
    pub(super) fn dex_names(&self) -> Vec<DexNames> {
        self.components.iter().map(DexNames::of).collect()
    }

    /// The DEX entries each component must rewrite, in component order and,
    /// within a component, in archive order followed by new entries, which
    /// take their names from `names` in that order.
    pub(super) fn dex_jobs(&self, names: &mut [DexNames]) -> Result<Vec<DexJob>> {
        let rewritten: HashSet<_> = self
            .dex
            .slots()
            .filter(|&(index, _, _)| self.dex.is_rewritten(index))
            .map(|(_, _, origin)| (origin.component, &origin.name))
            .collect();
        let mut jobs = Vec::new();
        let mut emitted = HashSet::new();
        for (dex_index, dex, origin) in self.dex.slots() {
            if origin.kind == DexSource::Removed
                || !self.components[origin.component.0].contains(origin.name.as_str())
                || !rewritten.contains(&(origin.component, &origin.name))
            {
                continue;
            }
            let component = origin.component;
            if !emitted.insert((component, &origin.name)) {
                continue;
            }
            let compression = self.components[component.0].dex_compression(origin.name.as_str())?;
            let name = if origin.kind == DexSource::Added {
                names[component.0].next()
            } else {
                EntryName::from(names[component.0].output(origin.name.as_str()))
            };
            if dex.header().version.is_container_format() {
                let members = self
                    .dex
                    .slots()
                    .filter(|(_, _, member)| {
                        member.kind != DexSource::Removed
                            && member.component == component
                            && member.name == origin.name
                    })
                    .map(|(index, dex, _)| members_for(index, dex))
                    .collect::<Result<Vec<_>>>()?
                    .into_iter()
                    .flatten()
                    .collect();
                jobs.push(DexJob {
                    members,
                    component,
                    source: Some(origin.name.clone()),
                    name,
                    compression,
                });
                continue;
            }
            for (ordinal, member) in members_for(dex_index, dex)?.into_iter().enumerate() {
                let (source, name) = if ordinal == 0 {
                    (Some(origin.name.clone()), name.clone())
                } else {
                    (None, names[component.0].next())
                };
                jobs.push(DexJob {
                    members: vec![member],
                    component,
                    source,
                    name,
                    compression,
                });
            }
        }
        Ok(jobs)
    }
}

fn members_for(index: DexIndex, dex: &reseam_dex::DexFile) -> Result<Vec<DexMember>> {
    Ok(match reseam_dex::split_to_fit(dex)? {
        None => vec![DexMember {
            dex_index: index,
            part: None,
        }],
        Some(parts) => parts
            .into_iter()
            .map(|part| DexMember {
                dex_index: index,
                part: Some(part),
            })
            .collect(),
    })
}
