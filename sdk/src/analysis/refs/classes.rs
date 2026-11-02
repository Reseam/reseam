// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use reseam_dex::{DexFile, FieldIdx, InstructionRef, MethodIdx, RefQuery, TypeIdx};
use reseam_model::analysis::{ClassRef, Member, References, Relation};

use super::Search;
use crate::analysis::Analysis;
use crate::analysis::code::{field_ref, method_ref, ref_kind};
use crate::analysis::hierarchy::Inheritance;
use crate::error::Result;

impl Analysis {
    pub(super) fn class_refs(&self, class: &ClassRef) -> Result<References> {
        let sites = self.search(|dex| {
            let types: HashSet<TypeIdx> = (0..dex.types().len())
                .map(|index| TypeIdx(index as u32))
                .filter(|&type_| element_type(dex, type_) == class.descriptor())
                .collect();
            let target = Member::Class(class.clone());
            (!types.is_empty()).then(|| Search {
                query: RefQuery::default(),
                matches: move |_: &DexFile, instruction: InstructionRef<'_>| {
                    types.contains(&instruction.type_ref()?).then_some(())?;
                    Some((ref_kind(instruction.opcode()?)?, Some(target.clone())))
                },
            })
        })?;
        let mut relations: Vec<Relation> = self
            .hierarchy()
            .children(class)
            .iter()
            .map(|(child, inheritance)| match inheritance {
                Inheritance::Extends => Relation::Subclass {
                    class: child.clone(),
                },
                Inheritance::Implements => Relation::Implementer {
                    class: child.clone(),
                },
            })
            .collect();
        relations.extend(self.typed_members(class));
        Ok(References { sites, relations })
    }

    /// Fields of type `class`, and methods that take or return it.
    fn typed_members(&self, class: &ClassRef) -> Vec<Relation> {
        let mut relations = Vec::new();
        for dex in self.dex().iter() {
            let Some(type_) = dex.find_type_idx(class.descriptor()) else {
                continue;
            };
            for index in 0..dex.fields().len() {
                let field = FieldIdx(index as u32);
                if dex.field_id(field).type_ == type_ {
                    relations.push(Relation::FieldOfType {
                        member: Member::Field(field_ref(dex, field)),
                    });
                }
            }
            for index in 0..dex.methods().len() {
                let method = MethodIdx(index as u32);
                let proto = dex.proto(dex.method_id(method).proto);
                if proto.return_type == type_ || proto.parameters.contains(&type_) {
                    relations.push(Relation::Signature {
                        method: method_ref(dex, method),
                    });
                }
            }
        }
        relations.dedup();
        relations
    }
}

/// The class a type names, through any array dimensions: `[[Lc;` is `Lc;`.
/// Primitive types and arrays of them name none and come back unchanged.
fn element_type(dex: &DexFile, type_: TypeIdx) -> String {
    dex.type_descriptor(type_)
        .trim_start_matches('[')
        .to_owned()
}
