// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use reseam_dex::{
    DexFile, FieldIdx, InstructionRef, MethodIdx, RefKey, RefQuery, StringIdx, TypeIdx,
};
use reseam_model::analysis::{Member, RefSite, References, Relation};

use super::{Search, string_search};
use crate::analysis::Analysis;
use crate::analysis::code::{field_ref, method_ref, ref_kind};
use crate::error::Result;

impl Analysis {
    pub(in crate::analysis) fn string_sites(&self, value: &str) -> Result<Vec<RefSite>> {
        self.search(|dex| {
            let string = dex.strings().find(value)?;
            Some(string_search(string))
        })
    }

    /// Instructions that load string constant `string` of each DEX.
    pub(in crate::analysis) fn strings_sites(
        &self,
        strings: &[(usize, StringIdx)],
    ) -> Result<Vec<RefSite>> {
        self.search_each(|dex_idx, _| {
            strings
                .iter()
                .find(|(dex, _)| *dex == dex_idx)
                .map(|&(_, string)| string_search(string))
        })
    }

    pub(in crate::analysis) fn number_sites(&self, value: i64) -> Result<Vec<RefSite>> {
        self.search(|_| {
            Some(Search {
                query: RefQuery::all_of([RefKey::Literal(value)]),
                matches: move |_: &DexFile, instruction: InstructionRef<'_>| {
                    (instruction.literal()? == value).then_some(())?;
                    Some((ref_kind(instruction.opcode()?)?, None))
                },
            })
        })
    }

    pub(super) fn resource_refs(&mut self, id: u32) -> Result<References> {
        let sites = self.number_sites(i64::from(id as i32))?;
        let mut relations: Vec<Relation> = {
            let xml = self.xml_index()?;
            xml.uses(id)
                .map(|element| Relation::Xml {
                    file: xml.files[element.file].path.clone(),
                    element: element.tag.clone(),
                })
                .collect()
        };
        let manifest = self.opened.apk.base().manifest();
        for (event, element) in manifest.events().iter().enumerate() {
            let reseam_apk::axml::AxmlEvent::StartElement { attributes, .. } = element else {
                continue;
            };
            if attributes.iter().any(|attribute| {
                attribute.value().kind == reseam_apk::ResValue::REFERENCE
                    && attribute.value().data == id
            }) {
                relations.push(Relation::Manifest {
                    element: manifest
                        .element_name(event)
                        .expect("a start element has a name")
                        .into_owned(),
                });
            }
        }
        Ok(References { sites, relations })
    }

    /// Calls and field accesses from outside `package` into it, by member.
    pub(super) fn package_sites(&self, package: &str) -> Result<Vec<RefSite>> {
        let prefix = format!("L{}/", package.replace('.', "/"));
        self.search(|dex| {
            let inside = |type_: TypeIdx| dex.type_descriptor(type_).starts_with(&prefix);
            let methods: HashSet<MethodIdx> = (0..dex.methods().len())
                .map(|index| MethodIdx(index as u32))
                .filter(|&method| inside(dex.method_id(method).class))
                .collect();
            let fields: HashSet<FieldIdx> = (0..dex.fields().len())
                .map(|index| FieldIdx(index as u32))
                .filter(|&field| inside(dex.field_id(field).class))
                .collect();
            let keys = methods
                .iter()
                .map(|&method| RefKey::Method(method))
                .chain(fields.iter().map(|&field| RefKey::Field(field)));
            let query = RefQuery::any_of(keys);
            (!query.is_empty()).then(|| Search {
                query,
                matches: move |dex: &DexFile, instruction: InstructionRef<'_>| {
                    let kind = ref_kind(instruction.opcode()?)?;
                    if let Some(method) = instruction.method_ref().filter(|m| methods.contains(m)) {
                        return Some((kind, Some(Member::Method(method_ref(dex, method)))));
                    }
                    let field = instruction.field_ref().filter(|f| fields.contains(f))?;
                    Some((kind, Some(Member::Field(field_ref(dex, field)))))
                },
            })
        })
        .map(|sites| {
            sites
                .into_iter()
                .filter(|site| !site.site.method.class.descriptor().starts_with(&prefix))
                .collect()
        })
    }
}
