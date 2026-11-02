// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use reseam_dex::{DexFile, FieldIdx, InstructionRef, MethodIdx, RefKey, RefQuery};
use reseam_model::analysis::{
    ClassRef, FieldRef, Member, MethodRef, RefKind, References, Relation,
};

use super::Search;
use crate::analysis::code::{field_ref, method_ref, ref_kind};
use crate::analysis::{Analysis, method_idx};
use crate::error::Result;

/// An interface method more classes implement than this is a callback type, like a Kotlin
/// lambda's `invoke`: a call through it says nothing about which implementation runs. Most
/// interfaces have one to three implementers; Kotlin's function interfaces have hundreds. In two
/// test apps, 42 of 1,531 and 88 of 4,640 interfaces have more than 20.
const SHARED_IMPLEMENTERS: usize = 20;

impl Analysis {
    pub(super) fn field_refs(&self, field: &FieldRef) -> Result<References> {
        let owners = self.inheriting(&field.class, |_| false);
        let sites = self.search(|dex| {
            let ids: HashSet<FieldIdx> = owners
                .iter()
                .filter_map(|owner| {
                    let class = dex.find_type_idx(owner.descriptor())?;
                    dex.find_field(class, &field.name, &field.type_)
                })
                .collect();
            (!ids.is_empty()).then(|| Search {
                query: RefQuery::any_of(ids.iter().map(|&id| RefKey::Field(id))),
                matches: move |dex: &DexFile, instruction: InstructionRef<'_>| {
                    let accessed = instruction.field_ref()?;
                    ids.contains(&accessed).then_some(())?;
                    let target = Member::Field(field_ref(dex, accessed));
                    Some((ref_kind(instruction.opcode()?)?, Some(target)))
                },
            })
        })?;
        Ok(References {
            sites,
            relations: Vec::new(),
        })
    }

    pub(super) fn method_refs(&self, method: &MethodRef) -> Result<References> {
        let overrides: Vec<MethodRef> = self
            .hierarchy()
            .descendants(&method.class)
            .into_iter()
            .filter(|class| self.defines(class, method))
            .map(|class| MethodRef {
                class,
                ..method.clone()
            })
            .collect();
        let (shared, overridden): (Vec<MethodRef>, Vec<MethodRef>) = self
            .overridden(method)
            .into_iter()
            .partition(|ancestor| self.implementers(&ancestor.class) > SHARED_IMPLEMENTERS);
        let mut owners = self.inheriting(&method.class, |class| self.defines(class, method));
        for ancestor in &overridden {
            owners.extend(self.inheriting(&ancestor.class, |class| {
                class == &method.class || self.defines(class, method)
            }));
        }
        owners.sort();
        owners.dedup();
        let sites = self.search(|dex| {
            let ids: HashSet<MethodIdx> = owners
                .iter()
                .filter_map(|owner| {
                    method_idx(
                        dex,
                        &MethodRef {
                            class: owner.clone(),
                            ..method.clone()
                        },
                    )
                })
                .collect();
            (!ids.is_empty()).then(|| Search {
                query: RefQuery::any_of(ids.iter().map(|&id| RefKey::Method(id))),
                matches: move |dex: &DexFile, instruction: InstructionRef<'_>| {
                    let called = instruction.method_ref()?;
                    ids.contains(&called)
                        .then(|| (RefKind::Call, Some(Member::Method(method_ref(dex, called)))))
                },
            })
        })?;
        Ok(References {
            sites,
            relations: overrides
                .into_iter()
                .map(|method| Relation::Override { method })
                .chain(
                    overridden
                        .into_iter()
                        .map(|method| Relation::Overridden { method }),
                )
                .chain(shared.into_iter().map(|method| Relation::Shared {
                    implementers: self.implementers(&method.class) as u32,
                    method,
                }))
                .collect(),
        })
    }

    /// The methods of the APK's own supertypes of `method`'s class that
    /// `method` overrides or implements.
    fn overridden(&self, method: &MethodRef) -> Vec<MethodRef> {
        let mut found = Vec::new();
        let mut seen = HashSet::new();
        let parents = |class: &ClassRef| -> Vec<ClassRef> {
            self.hierarchy()
                .get(class)
                .map(|info| {
                    info.superclass
                        .iter()
                        .chain(&info.interfaces)
                        .cloned()
                        .collect()
                })
                .unwrap_or_default()
        };
        let mut pending = parents(&method.class);
        while let Some(class) = pending.pop() {
            if !seen.insert(class.clone()) || self.hierarchy().get(&class).is_none() {
                continue;
            }
            pending.extend(parents(&class));
            if self.defines(&class, method) {
                found.push(MethodRef {
                    class,
                    ..method.clone()
                });
            }
        }
        found.sort();
        found
    }

    /// `class` and its descendants that `overrides` does not stop: the
    /// classes through which a reference reaches the member `class` declares.
    fn inheriting(&self, class: &ClassRef, overrides: impl Fn(&ClassRef) -> bool) -> Vec<ClassRef> {
        let mut owners = vec![class.clone()];
        let mut pending = vec![class.clone()];
        while let Some(parent) = pending.pop() {
            for (child, _) in self.hierarchy().children(&parent) {
                if !overrides(child) {
                    owners.push(child.clone());
                    pending.push(child.clone());
                }
            }
        }
        owners
    }

    /// How many classes of the APK name `class` as their superclass or an interface.
    fn implementers(&self, class: &ClassRef) -> usize {
        self.hierarchy().children(class).len()
    }

    /// Whether `class` defines a method with the name and prototype of `method`.
    fn defines(&self, class: &ClassRef, method: &MethodRef) -> bool {
        self.method(&MethodRef {
            class: class.clone(),
            ..method.clone()
        })
        .is_ok()
    }
}
