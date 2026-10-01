// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

mod value;

pub use value::{AttributeValue, attribute_symbols, infer_value, parse_attribute_value};

use super::NodeMetadata;

use super::text::{self, Attribute, Element, Name, Namespace, Node};
use super::{
    AAPT_NS, ANDROID_NS, AxmlAttribute, AxmlDocument, AxmlEvent, TOOLS_NS, android_attr_symbol,
    android_res_id,
};
use crate::error::{Result, invalid};
use crate::resources::{AttrFormats, ResourceScope, ResourceTable};
use crate::value::ResValue;

pub fn is_compiled_axml(data: &[u8]) -> bool {
    data.get(..2) == Some(&[0x03, 0x00])
}

/// Compiles XML with scoped namespaces and typed attribute values. Resource
/// names resolve through `resources`; new IDs are created in its local table.
pub fn compile_xml(text: &str, resources: Option<&mut ResourceScope<'_>>) -> Result<Vec<u8>> {
    build_document(text, resources)?.serialize()
}

pub fn build_document(
    text: &str,
    resources: Option<&mut ResourceScope<'_>>,
) -> Result<AxmlDocument> {
    build_element(&text::parse(text)?, resources)
}

struct BoundAttribute<'a> {
    uri: Option<String>,
    local: &'a str,
    name: Option<u32>,
    res_id: Option<u32>,
    text: &'a str,
}

struct ElementName<'a> {
    uri: Option<String>,
    local: &'a str,
}

enum Prepared<'a> {
    Start {
        name: ElementName<'a>,
        attributes: Vec<BoundAttribute<'a>>,
        shadows: Vec<Namespace>,
    },
    End {
        name: ElementName<'a>,
        shadows: Vec<Namespace>,
    },
    Text(&'a str),
}

enum Work<'a> {
    Enter(&'a Element, Vec<Namespace>),
    Leave(ElementName<'a>, Vec<Namespace>),
    Text(&'a str),
}

pub(super) fn build_element(
    root: &Element,
    mut resources: Option<&mut ResourceScope<'_>>,
) -> Result<AxmlDocument> {
    let mut doc = AxmlDocument::new(crate::StringEncoding::Utf8);
    let PreparedDocument { namespaces, events } = prepare(root, &mut doc, resources.as_deref())?;
    let namespace_indices = namespaces
        .iter()
        .map(|decl| namespace_event(&mut doc, decl))
        .collect::<Vec<_>>();
    doc.elements.extend(
        namespace_indices
            .iter()
            .map(|&(prefix, uri)| AxmlEvent::StartNamespace {
                metadata: NodeMetadata::default(),
                prefix,
                uri,
            }),
    );
    for event in events {
        match event {
            Prepared::Start {
                name,
                attributes,
                shadows,
            } => {
                for decl in &shadows {
                    let (prefix, uri) = namespace_event(&mut doc, decl);
                    doc.elements.push(AxmlEvent::StartNamespace {
                        metadata: NodeMetadata::default(),
                        prefix,
                        uri,
                    });
                }
                let namespace = name.uri.as_deref().map(|uri| doc.intern_string(uri));
                let name = doc.intern_string(name.local);
                let mut attributes = attributes
                    .into_iter()
                    .map(|attr| {
                        let namespace = attr.uri.as_deref().map(|uri| doc.intern_string(uri));
                        let name = attr.name.unwrap_or_else(|| doc.intern_string(attr.local));
                        let value = match match attr.res_id {
                            Some(id) => {
                                parse_attribute_value(attr.text, id, resources.as_deref_mut())
                            }
                            None => infer_value(attr.text, resources.as_deref_mut()),
                        }? {
                            AttributeValue::Value(value) => value,
                            AttributeValue::Text => ResValue::string(doc.intern_string(attr.text)),
                        };
                        Ok(AxmlAttribute::new(namespace, name, value))
                    })
                    .collect::<Result<Vec<_>>>()?;
                attributes.sort_by_key(|attr| doc.resource_id_for(attr.name).unwrap_or(u32::MAX));
                doc.elements.push(AxmlEvent::StartElement {
                    metadata: NodeMetadata::default(),
                    namespace,
                    name,
                    attributes,
                });
            }
            Prepared::End { name, shadows } => {
                let namespace = name.uri.as_deref().map(|uri| doc.intern_string(uri));
                let name = doc.intern_string(name.local);
                doc.elements.push(AxmlEvent::EndElement {
                    metadata: NodeMetadata::default(),
                    namespace,
                    name,
                });
                for decl in shadows.iter().rev() {
                    let (prefix, uri) = namespace_event(&mut doc, decl);
                    doc.elements.push(AxmlEvent::EndNamespace {
                        metadata: NodeMetadata::default(),
                        prefix,
                        uri,
                    });
                }
            }
            Prepared::Text(text) => {
                let text = doc.intern_string(text);
                doc.elements.push(AxmlEvent::Text {
                    metadata: NodeMetadata::default(),
                    text,
                    value: ResValue::string(text),
                });
            }
        }
    }
    doc.elements
        .extend(
            namespace_indices
                .into_iter()
                .map(|(prefix, uri)| AxmlEvent::EndNamespace {
                    metadata: NodeMetadata::default(),
                    prefix,
                    uri,
                }),
        );
    Ok(doc)
}

struct PreparedDocument<'a> {
    namespaces: Vec<Namespace>,
    events: Vec<Prepared<'a>>,
}

fn prepare<'a>(
    root: &'a Element,
    doc: &mut AxmlDocument,
    resources: Option<&ResourceScope<'_>>,
) -> Result<PreparedDocument<'a>> {
    let mut namespaces: Vec<Namespace> = Vec::new();
    let mut prepared = Vec::new();
    let mut work = vec![Work::Enter(
        root,
        vec![Namespace {
            prefix: Some("android".to_string()),
            uri: ANDROID_NS.to_string(),
        }],
    )];
    while let Some(next) = work.pop() {
        match next {
            Work::Leave(name, shadows) => prepared.push(Prepared::End { name, shadows }),
            Work::Text(text) => {
                if !text.trim().is_empty() {
                    prepared.push(Prepared::Text(text));
                }
            }
            Work::Enter(element, mut scope) => {
                scope.extend(element.declarations().cloned());
                for attr in &element.attributes {
                    let decl = match attr {
                        Attribute::Namespace(decl) => Some(decl.clone()),
                        Attribute::Value { name, .. }
                            if name.prefix.as_deref() == Some("android") =>
                        {
                            Some(Namespace {
                                prefix: name.prefix.clone(),
                                uri: resolve_namespace(name, &scope, NameUse::Attribute)?
                                    .expect("qualified attribute has a namespace"),
                            })
                        }
                        Attribute::Value { .. } => None,
                    };
                    if let Some(decl) = decl
                        && !namespaces
                            .iter()
                            .any(|existing| existing.prefix == decl.prefix)
                    {
                        namespaces.push(decl);
                    }
                }
                let uri = resolve_namespace(&element.name, &scope, NameUse::Element)?;
                if element.name.local == "attr" && uri.as_deref() == Some(AAPT_NS) {
                    return Err(invalid(
                        "axml compiler",
                        "<aapt:attr> must be extracted as an inline resource before compilation",
                    ));
                }
                let shadows = element
                    .declarations()
                    .filter(|decl| {
                        namespaces
                            .iter()
                            .any(|global| global.prefix == decl.prefix && global.uri != decl.uri)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                let attributes = bind_attributes(element, &scope, doc, resources)?;
                prepared.push(Prepared::Start {
                    name: ElementName {
                        uri: uri.clone(),
                        local: &element.name.local,
                    },
                    attributes,
                    shadows: shadows.clone(),
                });
                work.push(Work::Leave(
                    ElementName {
                        uri,
                        local: &element.name.local,
                    },
                    shadows,
                ));
                work.extend(element.children.iter().rev().map(|child| match child {
                    Node::Element(child) => Work::Enter(child, scope.clone()),
                    Node::Text(text) => Work::Text(text),
                }));
            }
        }
    }
    Ok(PreparedDocument {
        namespaces,
        events: prepared,
    })
}

fn bind_attributes<'a>(
    element: &'a Element,
    scope: &[Namespace],
    doc: &mut AxmlDocument,
    resources: Option<&ResourceScope<'_>>,
) -> Result<Vec<BoundAttribute<'a>>> {
    Ok(element
        .values()
        .map(|(name, value)| {
            let uri = resolve_namespace(name, scope, NameUse::Attribute)?;
            if uri.as_deref() == Some(TOOLS_NS) {
                return Ok(None);
            }
            let res_id = attribute_resource_id(
                uri.as_deref(),
                &name.local,
                resources.map(ResourceScope::table),
            )?;
            let index = res_id.map(|id| doc.intern_attribute_name(&name.local, id));
            Ok(Some(BoundAttribute {
                uri,
                local: &name.local,
                name: index,
                res_id,
                text: value,
            }))
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect())
}

fn namespace_event(doc: &mut AxmlDocument, decl: &Namespace) -> (Option<u32>, u32) {
    (
        decl.prefix
            .as_deref()
            .map(|prefix| doc.intern_string(prefix)),
        doc.intern_string(&decl.uri),
    )
}

#[derive(Clone, Copy)]
enum NameUse {
    Element,
    Attribute,
}

fn resolve_namespace(name: &Name, scope: &[Namespace], use_of: NameUse) -> Result<Option<String>> {
    if name.prefix.is_none() && matches!(use_of, NameUse::Attribute) {
        return Ok(None);
    }
    match (
        name.prefix.as_deref(),
        text::namespace(scope.iter(), name.prefix.as_deref()),
    ) {
        (_, Some("")) | (None, None) => Ok(None),
        (_, Some(uri)) => Ok(Some(uri.to_string())),
        (Some(prefix), None) => Err(invalid(
            "XML namespace",
            format!("{}: undeclared prefix {prefix}", name.qualified()),
        )),
    }
}

pub(super) fn attribute_resource_id(
    uri: Option<&str>,
    local: &str,
    resources: Option<&ResourceTable>,
) -> Result<Option<u32>> {
    match uri {
        None => Ok(None),
        Some(ANDROID_NS) => android_resource("attr", local).map(Some),
        Some(_) => resources
            .map(|table| table.find_resource_id("attr", local))
            .transpose()?
            .flatten()
            .map(Some)
            .ok_or_else(|| invalid("XML attribute", format!("the app has no attr/{local}"))),
    }
}

fn android_resource(type_name: &str, name: &str) -> Result<u32> {
    android_res_id(type_name, name).ok_or_else(|| {
        invalid(
            "framework resource",
            format!("android:{type_name}/{name} is not public"),
        )
    })
}
