// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use super::super::{ClassLocation, MethodLocation, PatchContext};
use super::method_location;
use crate::error::Result;
use reseam_dex::{InstructionPattern, MethodIdx, RefKey, RefQuery, StringIdx, TypeIdx};

impl PatchContext<'_> {
    /// Every method whose prototype satisfies each given filter (exact return
    /// type, exact parameter list, a parameter of `contains` type anywhere),
    /// found through the id tables; only the classes defining them are read.
    /// A DEX missing any of the named types has no matches.
    pub fn find_methods_by_proto(
        &self,
        return_type: Option<&str>,
        parameters: Option<&[&str]>,
        contains: Option<&str>,
    ) -> Result<Vec<MethodLocation>> {
        self.scan_all("methods by prototype", |dex_idx, dex| {
            let resolve = |t: &str| dex.find_type_idx(t).ok_or(());
            let resolved = (|| {
                Ok::<_, ()>((
                    return_type.map(resolve).transpose()?,
                    parameters
                        .map(|types| {
                            types
                                .iter()
                                .map(|t| resolve(t))
                                .collect::<std::result::Result<Vec<_>, ()>>()
                        })
                        .transpose()?,
                    contains.map(resolve).transpose()?,
                ))
            })();
            let Ok((return_type, parameters, contains)) = resolved else {
                return Ok(Vec::new());
            };
            let matching: Vec<bool> = dex
                .prototypes()
                .iter()
                .map(|proto| {
                    return_type.is_none_or(|t| proto.return_type == t)
                        && parameters
                            .as_deref()
                            .is_none_or(|types| proto.parameters.as_slice() == types)
                        && contains.is_none_or(|t| proto.parameters.contains(&t))
                })
                .collect();
            let methods: Vec<MethodIdx> = dex
                .methods()
                .iter()
                .enumerate()
                .filter(|(_, method)| matching[method.proto.0 as usize])
                .map(|(index, _)| MethodIdx(index as u32))
                .collect();
            let hits = dex.scan_method_definitions(&methods, |view| Ok(Some(view.hit())))?;
            Ok(hits
                .iter()
                .map(|hit| method_location(dex_idx, hit))
                .collect())
        })
    }

    pub fn find_methods_by_name(&self, method_name: &str) -> Result<Vec<MethodLocation>> {
        self.scan_all("methods by name", |dex_idx, dex| {
            let Some(name) = dex.strings().find(method_name) else {
                return Ok(Vec::new());
            };
            let methods: Vec<MethodIdx> = dex
                .methods()
                .iter()
                .enumerate()
                .filter(|(_, method)| method.name == name)
                .map(|(index, _)| MethodIdx(index as u32))
                .collect();
            let hits = dex.scan_method_definitions(&methods, |view| Ok(Some(view.hit())))?;
            Ok(hits
                .iter()
                .map(|hit| method_location(dex_idx, hit))
                .collect())
        })
    }

    pub fn find_methods_by_strings(&self, strings: &[&str]) -> Result<Vec<MethodLocation>> {
        self.scan_all("methods by strings", |dex_idx, dex| {
            let hits = dex.find_methods_by_strings(strings)?;
            Ok(hits
                .iter()
                .map(|hit| method_location(dex_idx, hit))
                .collect())
        })
    }

    pub fn find_methods_with_opcodes(
        &self,
        opcodes: &[InstructionPattern],
    ) -> Result<Vec<MethodLocation>> {
        self.scan_all("methods by opcodes", |dex_idx, dex| {
            let hits = dex.find_methods_with_opcodes(opcodes)?;
            Ok(hits
                .iter()
                .map(|hit| method_location(dex_idx, hit))
                .collect())
        })
    }

    /// Visits every method in the APK in DEX, class, then member order
    /// without collecting them first.
    pub fn for_each_method(&self, mut visit: impl FnMut(MethodLocation)) -> Result<()> {
        for (dex_idx, dex) in self.dex().iter().enumerate() {
            dex.scan_methods_find(&RefQuery::default(), |view| {
                visit(method_location(dex_idx, &view.hit()));
                Ok(None::<()>)
            })?;
        }
        Ok(())
    }

    pub fn find_classes_with_instance_field(&self, field_type: &str) -> Result<Vec<ClassLocation>> {
        self.scan_all("classes with instance field", |dex_idx, dex| {
            let Some(wanted) = dex.find_type_idx(field_type) else {
                return Ok(Vec::new());
            };
            let mut matches = Vec::new();
            for class_idx in 0..dex.classes().len() {
                if dex
                    .decode_class_fields(class_idx)?
                    .is_some_and(|(_, fields)| {
                        fields.iter().any(|f| dex.field_id(f.field).type_ == wanted)
                    })
                {
                    matches.push(ClassLocation { dex_idx, class_idx });
                }
            }
            Ok(matches)
        })
    }

    pub fn find_classes_implementing(&self, interface: &str) -> Result<Vec<ClassLocation>> {
        self.scan_all("classes implementing", |dex_idx, dex| {
            let Some(wanted) = dex.find_type_idx(interface) else {
                return Ok(Vec::new());
            };
            let classes = dex.classes();
            Ok((0..classes.len())
                .filter(|&class_idx| classes.interfaces(class_idx).contains(&wanted))
                .map(|class_idx| ClassLocation { dex_idx, class_idx })
                .collect())
        })
    }

    /// Level by level across every DEX, since a subclass can live in another file.
    pub fn find_classes_extending(&self, superclass: &str) -> Result<Vec<ClassLocation>> {
        let mut seen = HashSet::from([superclass.to_owned()]);
        let mut parents = vec![superclass.to_owned()];
        let mut found = Vec::new();
        while !parents.is_empty() {
            let children = self.scan_all("classes extending", |dex_idx, dex| {
                let wanted: HashSet<TypeIdx> = parents
                    .iter()
                    .filter_map(|parent| dex.find_type_idx(parent))
                    .collect();
                if wanted.is_empty() {
                    return Ok(Vec::new());
                }
                Ok(dex
                    .classes()
                    .headers()
                    .enumerate()
                    .filter(|(_, header)| header.superclass.is_some_and(|s| wanted.contains(&s)))
                    .map(|(class_idx, header)| {
                        let descriptor = dex.type_descriptor(header.class_type).into_owned();
                        (ClassLocation { dex_idx, class_idx }, descriptor)
                    })
                    .collect())
            })?;
            parents = Vec::new();
            for (location, descriptor) in children {
                if seen.insert(descriptor.clone()) {
                    found.push(location);
                    parents.push(descriptor);
                }
            }
        }
        Ok(found)
    }

    /// Methods referencing a string that starts with `prefix`, found through
    /// the string pool's sorted range and the reference index.
    pub fn find_methods_by_string_prefix(&self, prefix: &str) -> Result<Vec<MethodLocation>> {
        self.scan_all("methods by string prefix", |dex_idx, dex| {
            let matches: HashSet<StringIdx> =
                dex.strings().with_prefix(prefix).into_iter().collect();
            if matches.is_empty() {
                return Ok(Vec::new());
            }
            let query = RefQuery::any_of(matches.iter().map(|&s| RefKey::String(s)));
            let hits = dex.scan_methods_collect(&query, |view| {
                let found = view.any_instruction(|insn| {
                    insn.string_ref().is_some_and(|s| matches.contains(&s))
                })?;
                Ok(found.then(|| view.hit()))
            })?;
            Ok(hits
                .iter()
                .map(|hit| method_location(dex_idx, hit))
                .collect())
        })
    }
}
