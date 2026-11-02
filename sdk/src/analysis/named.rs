// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Classes and methods by a name the DEX still holds for them. Each kind of
//! hint comes from a string, so the strings find the candidates and the hint
//! rules confirm them.

use std::collections::BTreeSet;

use reseam_dex::StringIdx;
use reseam_model::analysis::{ClassRef, Member, NamedMember};

use super::Analysis;
use crate::error::Result;

impl Analysis {
    /// Every class and method with a hint called `name`.
    pub fn named(&self, name: &str) -> Result<Vec<NamedMember>> {
        let mut found = BTreeSet::new();
        for site in self.string_sites(name)? {
            self.confirm_class(&site.site.method.class, name, &mut found)?;
            for hint in self.method_hints(&site.site.method)? {
                if hint.name == name {
                    found.insert(NamedMember {
                        member: Member::Method(site.site.method.clone()),
                        hint,
                    });
                }
            }
        }
        let prefixes = [format!("{name}("), format!("{name}{{")];
        let mut starts = Vec::new();
        for (dex_idx, dex) in self.dex().iter().enumerate() {
            for (index, value) in dex.strings().iter().enumerate() {
                if prefixes
                    .iter()
                    .any(|prefix| value.starts_with(prefix.as_str()))
                {
                    starts.push((dex_idx, StringIdx(index as u32)));
                }
            }
        }
        for site in self.strings_sites(&starts)? {
            if site.site.method.name == "toString" {
                self.confirm_class(&site.site.method.class, name, &mut found)?;
            }
        }
        for class in self.classes_from_source(name) {
            self.confirm_class(&class, name, &mut found)?;
        }
        Ok(found.into_iter().collect())
    }

    fn confirm_class(
        &self,
        class: &ClassRef,
        name: &str,
        found: &mut BTreeSet<NamedMember>,
    ) -> Result<()> {
        for hint in self.class_hints(class)? {
            if hint.name == name {
                found.insert(NamedMember {
                    member: Member::Class(class.clone()),
                    hint,
                });
            }
        }
        Ok(())
    }

    /// Classes whose `SourceFile` is `name.kt` or `name.java`.
    fn classes_from_source(&self, name: &str) -> Vec<ClassRef> {
        let mut classes = Vec::new();
        for dex in self.dex().iter() {
            let files: Vec<StringIdx> = [format!("{name}.kt"), format!("{name}.java")]
                .iter()
                .filter_map(|file| dex.strings().find(file))
                .collect();
            if files.is_empty() {
                continue;
            }
            for header in dex.classes().headers() {
                if header.source_file.is_some_and(|file| files.contains(&file)) {
                    let descriptor = dex.type_descriptor(header.class_type);
                    classes.push(
                        descriptor
                            .parse()
                            .expect("a DEX class type is a class descriptor"),
                    );
                }
            }
        }
        classes
    }
}
