// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeSet;

use reseam_dex::{
    ClassDef, CodeItem, DeclarationKind, DexFile, EncodedField, SmaliLine, SmaliNames,
    access_keywords, disassemble,
};
use reseam_model::analysis::{
    BodyLine, ClassRef, ClassView, ComponentView, FieldRef, FieldView, LineKind, Member, MethodRef,
    MethodSummary, MethodView, RefKind, RefTarget, ResourceId,
};

use super::Analysis;
use super::code::{body, encoded_method, field_ref, method_ref, named_member};
use super::hierarchy::class_ref;
use super::hints::class_hints;
use super::manifest::components;
use crate::error::{HostError, Result};

impl Analysis {
    /// One method's body as smali, each instruction with what it names.
    pub fn show_method(&self, method: &MethodRef) -> Result<MethodView> {
        let (dex, class, idx) = self.method(method)?;
        let encoded = encoded_method(&class, idx).expect("Analysis::method found it defined");
        let mut hints = class_hints(dex, &method.class, &class);
        hints.extend(self.method_hints(method)?);
        let code = body(&class, idx);
        Ok(MethodView {
            method: method.clone(),
            access: access_keywords(encoded.access_flags, DeclarationKind::Method),
            registers: code.map_or(0, CodeItem::registers_size),
            parameter_registers: code.map_or(0, CodeItem::ins_size),
            hints,
            lines: code.map_or(Ok(Vec::new()), |code| body_lines(dex, code))?,
        })
    }

    /// A class's declaration and members.
    pub fn show_class(&self, class: &ClassRef) -> Result<ClassView> {
        let (dex, def) = self.class(class)?;
        let mut methods = Vec::new();
        if let Some(data) = def.class_data.as_deref() {
            for method in data.direct_methods.iter().chain(&data.virtual_methods) {
                let reference = method_ref(dex, method.method);
                methods.push(MethodSummary {
                    hints: self.method_hints(&reference)?,
                    method: reference,
                    access: access_keywords(method.access_flags, DeclarationKind::Method),
                });
            }
        }
        Ok(ClassView {
            class: class.clone(),
            access: access_keywords(def.access_flags, DeclarationKind::Class),
            superclass: def
                .superclass
                .map(|type_| class_ref(&dex.type_descriptor(type_))),
            interfaces: def
                .interfaces
                .iter()
                .map(|&type_| class_ref(&dex.type_descriptor(type_)))
                .collect(),
            source_file: def
                .source_file
                .map(|file| dex.strings().get(file).into_owned()),
            hints: class_hints(dex, class, &def),
            fields: fields(dex, &def)?,
            methods,
        })
    }

    /// A field's declaration in its class.
    pub fn show_field(&self, field: &FieldRef) -> Result<FieldView> {
        let (dex, def) = self.class(&field.class)?;
        fields(dex, &def)?
            .into_iter()
            .find(|view| view.field == *field)
            .ok_or_else(|| HostError::UnknownMember(Member::Field(field.clone())))
    }

    /// The decoded manifest of the base APK.
    pub fn manifest(&self) -> Result<String> {
        let apk = &self.opened.apk;
        apk.base()
            .manifest()
            .to_xml(apk.base().loaded_resources())
            .map_err(|error| self.opened.error(error))
    }

    /// The component `class` runs: its manifest entry, the layouts its class
    /// loads, and the code that names its class to start it.
    pub fn show_component(&mut self, class: &ClassRef) -> Result<ComponentView> {
        let (kind, manifest) = {
            let base = self.opened.apk.base();
            let component = components(base.manifest())
                .into_iter()
                .find(|component| component.class == *class)
                .ok_or_else(|| HostError::UnknownComponent(class.clone()))?;
            let xml = base
                .manifest()
                .element_to_xml(component.element, base.loaded_resources())
                .map_err(|error| self.opened.error(error))?;
            (component.kind, xml)
        };
        let started_by = self
            .refs(&RefTarget::Member(Member::Class(class.clone())))?
            .sites
            .into_iter()
            .filter(|site| site.kind == RefKind::ConstClass)
            .map(|site| site.site)
            .collect();
        Ok(ComponentView {
            kind,
            class: class.clone(),
            manifest,
            layouts: self.layouts_loaded_by(class)?,
            started_by,
        })
    }

    /// The layout resources whose ids the methods of `class` load. A
    /// component declared by a class the APK does not define loads none.
    fn layouts_loaded_by(&self, class: &ClassRef) -> Result<Vec<ResourceId>> {
        if self.hierarchy().get(class).is_none() {
            return Ok(Vec::new());
        }
        let (_, def) = self.class(class)?;
        let literals: BTreeSet<u32> = def
            .class_data
            .iter()
            .flat_map(|data| data.direct_methods.iter().chain(&data.virtual_methods))
            .filter_map(|method| method.code())
            .flat_map(CodeItem::instructions)
            .filter_map(|instruction| u32::try_from(instruction.literal()?).ok())
            .collect();
        let mut layouts = Vec::new();
        for id in literals {
            if let Some(name) = self.resource_name(id)?
                && name.type_name == "layout"
            {
                layouts.push(ResourceId {
                    id,
                    name: Some(name.to_string()),
                });
            }
        }
        Ok(layouts)
    }
}

fn body_lines(dex: &DexFile, code: &CodeItem) -> Result<Vec<BodyLine>> {
    let instructions = code.instructions();
    Ok(disassemble(dex, code)?
        .into_iter()
        .map(|line| match line {
            SmaliLine::Label(text) => BodyLine {
                kind: LineKind::Label,
                index: None,
                text,
                refers: None,
            },
            SmaliLine::Directive(text) => BodyLine {
                kind: LineKind::Directive,
                index: None,
                text,
                refers: None,
            },
            SmaliLine::Instruction { index, text } => {
                let refers = named_member(dex, &instructions[index]);
                BodyLine {
                    kind: LineKind::Instruction,
                    index: Some(
                        u32::try_from(index).expect("bodies hold fewer than 2^32 instructions"),
                    ),
                    text,
                    refers,
                }
            }
        })
        .collect())
}

fn fields(dex: &DexFile, def: &ClassDef) -> Result<Vec<FieldView>> {
    let names = SmaliNames::new(dex);
    let Some(data) = def.class_data.as_deref() else {
        return Ok(Vec::new());
    };
    data.static_fields
        .iter()
        .chain(&data.instance_fields)
        .map(|field: &EncodedField| {
            Ok(FieldView {
                field: field_ref(dex, field.field),
                access: access_keywords(field.access_flags, DeclarationKind::Field),
                initial: def
                    .static_values
                    .get(&field.field)
                    .map(|value| names.value(value))
                    .transpose()?,
            })
        })
        .collect()
}
