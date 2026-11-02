// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! A class's declarations in smali's notation, naming everything they refer to,
//! so the same declaration reads the same whichever DEX holds it. Line numbers and
//! local names are left out: they move when unrelated code above them changes.
//! So is `final` on methods nothing can override (static, private, constructors):
//! ART ignores it there, and R8 clears it depending on how other code calls the method.

use std::collections::BTreeSet;
use std::fmt::Write;

use super::{DeclarationKind, SmaliLine, SmaliNames, access_keywords, disassemble};
use crate::error::Result;
use crate::file::DexFile;
use crate::model::access_flags::AccessFlags;
use crate::model::annotation::{AnnotationItem, AnnotationVisibility, AnnotationsDirectory};
use crate::model::class::{ClassDef, EncodedField, EncodedMethod, MethodKind};
use crate::model::{FieldIdx, MethodIdx, Pool, StringIdx, TypeIdx};
use crate::references;

/// What a declaration refers to: classes and strings, fields and methods by descriptor.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct References {
    pub types: BTreeSet<String>,
    pub fields: BTreeSet<String>,
    pub methods: BTreeSet<String>,
    pub strings: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    pub text: String,
    pub references: References,
}

/// A field or method: its descriptor, such as `Lpkg/Owner;->name:I`, its flags as
/// declared, and its declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberDeclaration {
    pub descriptor: String,
    pub flags: AccessFlags,
    pub declaration: Declaration,
}

/// Class `class_idx` of `dex`: the header (flags, superclass, interfaces and
/// annotations), then each field with its initial value and each method with its body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassDeclarations {
    pub descriptor: String,
    pub superclass: Option<String>,
    pub interfaces: Vec<String>,
    pub header: Declaration,
    pub fields: Vec<MemberDeclaration>,
    pub methods: Vec<MemberDeclaration>,
}

pub fn class_declarations(dex: &DexFile, class_idx: usize) -> Result<ClassDeclarations> {
    let class = dex.classes().read(class_idx, dex.parse_options())?;
    let annotations = class
        .annotations
        .as_ref()
        .map(|metadata| metadata.read())
        .transpose()?;
    let annotations = annotations.as_deref();
    let writer = Writer {
        dex,
        names: SmaliNames::new(dex),
    };
    let data = class.class_data.as_deref();
    Ok(ClassDeclarations {
        descriptor: dex.type_descriptor(class.class_type).into_owned(),
        superclass: class
            .superclass
            .map(|type_| dex.type_descriptor(type_).into_owned()),
        interfaces: class
            .interfaces
            .iter()
            .map(|type_| dex.type_descriptor(*type_).into_owned())
            .collect(),
        header: writer.header(&class, annotations)?,
        fields: data
            .into_iter()
            .flat_map(|data| data.static_fields.iter().chain(&data.instance_fields))
            .map(|field| writer.field(&class, field, annotations))
            .collect::<Result<_>>()?,
        methods: data
            .into_iter()
            .flat_map(|data| data.direct_methods.iter().chain(&data.virtual_methods))
            .map(|method| writer.method(method, annotations))
            .collect::<Result<_>>()?,
    })
}

struct Writer<'a> {
    dex: &'a DexFile,
    names: SmaliNames<'a>,
}

impl Writer<'_> {
    fn header(
        &self,
        class: &ClassDef,
        annotations: Option<&AnnotationsDirectory>,
    ) -> Result<Declaration> {
        let mut found = Vec::new();
        let mut text = format!(
            ".class {} {}",
            access_keywords(class.access_flags, DeclarationKind::Class),
            self.names.type_(class.class_type),
        );
        let supertypes = class.superclass.iter().map(|type_| (".super", type_));
        let interfaces = class.interfaces.iter().map(|type_| (".implements", type_));
        for (directive, type_) in supertypes.chain(interfaces) {
            found.push((Pool::Type, type_.0));
            write!(text, "\n{directive} {}", self.names.type_(*type_))
                .expect("writing to a String");
        }
        self.annotations(
            &mut text,
            &mut found,
            annotations.map_or(&[], |dir| &dir.class),
        )?;
        self.declaration(text, found)
    }

    fn field(
        &self,
        class: &ClassDef,
        field: &EncodedField,
        annotations: Option<&AnnotationsDirectory>,
    ) -> Result<MemberDeclaration> {
        let mut found = Vec::new();
        let descriptor = self.dex.field_descriptor(field.field);
        let mut text = format!(
            ".field {} {descriptor}",
            access_keywords(field.access_flags, DeclarationKind::Field),
        );
        found.push((Pool::Field, field.field.0));
        if let Some(value) = class.static_values.get(&field.field) {
            references::encoded_value(&mut sink(&mut found), value);
            write!(text, " = {}", self.names.value(value)?).expect("writing to a String");
        }
        let items = annotations.map_or(&[][..], |dir| member_items(&dir.fields, field.field));
        self.annotations(&mut text, &mut found, items)?;
        Ok(MemberDeclaration {
            descriptor,
            flags: field.access_flags,
            declaration: self.declaration(text, found)?,
        })
    }

    fn method(
        &self,
        method: &EncodedMethod,
        annotations: Option<&AnnotationsDirectory>,
    ) -> Result<MemberDeclaration> {
        let mut found = Vec::new();
        let descriptor = self.dex.method_descriptor(method.method);
        let flags = match MethodKind::of(method.access_flags) {
            MethodKind::Direct => method.access_flags & !AccessFlags::FINAL,
            MethodKind::Virtual => method.access_flags,
        };
        let mut text = format!(
            ".method {} {descriptor}",
            access_keywords(flags, DeclarationKind::Method),
        );
        found.push((Pool::Method, method.method.0));
        let items = annotations.map_or(&[][..], |dir| member_items(&dir.methods, method.method));
        self.annotations(&mut text, &mut found, items)?;
        let parameters =
            annotations.map_or(&[][..], |dir| member_items(&dir.parameters, method.method));
        for (index, items) in parameters.iter().enumerate() {
            write!(text, "\n.param {index}").expect("writing to a String");
            self.annotations(&mut text, &mut found, items)?;
        }
        if let Some(code) = method.code() {
            write!(text, "\n.registers {}", code.registers_size()).expect("writing to a String");
            references::code_body(&mut sink(&mut found), code);
            for line in disassemble(self.dex, code)? {
                let (SmaliLine::Label(line)
                | SmaliLine::Instruction { text: line, .. }
                | SmaliLine::Directive(line)) = line;
                write!(text, "\n{line}").expect("writing to a String");
            }
        }
        Ok(MemberDeclaration {
            descriptor,
            flags: method.access_flags,
            declaration: self.declaration(text, found)?,
        })
    }

    fn annotations(
        &self,
        text: &mut String,
        found: &mut Vec<(Pool, u32)>,
        items: &[AnnotationItem],
    ) -> Result<()> {
        for item in items {
            references::annotation_item(&mut sink(found), item);
            let visibility = match item.visibility {
                AnnotationVisibility::Build => "build",
                AnnotationVisibility::Runtime => "runtime",
                AnnotationVisibility::System => "system",
            };
            let elements = item
                .elements
                .iter()
                .map(|element| (element.name, &element.value));
            let annotation = self.names.annotation(item.type_, elements)?;
            write!(text, "\n.annotation {visibility} {annotation}").expect("writing to a String");
        }
        Ok(())
    }

    /// Resolves what `found` collected into names, through prototypes, call sites and method handles.
    fn declaration(&self, text: String, found: Vec<(Pool, u32)>) -> Result<Declaration> {
        let mut pending = found;
        let mut references = References::default();
        while let Some((pool, idx)) = pending.pop() {
            match pool {
                Pool::String => {
                    references
                        .strings
                        .insert(self.dex.strings().get(StringIdx(idx)).into_owned());
                }
                Pool::Type => {
                    references
                        .types
                        .insert(self.dex.type_descriptor(TypeIdx(idx)).into_owned());
                }
                Pool::Field => {
                    references
                        .fields
                        .insert(self.dex.field_descriptor(FieldIdx(idx)));
                }
                Pool::Method => {
                    references
                        .methods
                        .insert(self.dex.method_descriptor(MethodIdx(idx)));
                }
                Pool::Proto => {
                    let proto = self.dex.prototypes.get(idx as usize);
                    pending.extend(
                        std::iter::once(proto.return_type)
                            .chain(proto.parameters.iter().copied())
                            .map(|type_| (Pool::Type, type_.0)),
                    );
                }
                Pool::CallSite | Pool::MethodHandle => {
                    references::id_entry(
                        &mut |pool, idx| pending.push((pool, idx)),
                        self.dex,
                        pool,
                        idx,
                    )?;
                }
            }
        }
        Ok(Declaration { text, references })
    }
}

fn sink(found: &mut Vec<(Pool, u32)>) -> impl FnMut(Pool, u32) + '_ {
    |pool, idx| found.push((pool, idx))
}

/// The entry `member` has in an annotations directory list, empty when it has none.
fn member_items<K: Copy + PartialEq, T>(items: &[(K, Vec<T>)], member: K) -> &[T] {
    items
        .iter()
        .find(|(key, _)| *key == member)
        .map_or(&[], |(_, items)| items)
}
