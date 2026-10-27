// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod declare;

use super::bind::attribute_resource_id;
use super::text::{self, Element, Namespace, Node};
use super::value::{AttributeValue, parse_value};
use super::{ANDROID_NS, AxmlAttribute, AxmlDocument, AxmlEvent, NodeMetadata, TOOLS_NS};
use crate::error::Result;
use crate::res_value::ResValue;
use crate::resources::ResourceScope;
use crate::string_pool::StringEncoding;
use declare::{NameUse, declare, namespace_event, resolve_namespace, scoped};

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

/// Compiles in two passes over the tree, as aapt lays out a document. The
/// first collects the namespaces the root declares and binds every attribute
/// name that has a resource id, so those names lead the string pool where the
/// resource map covers them; the second emits the events.
pub(super) fn build_element(
    root: &Element,
    mut resources: Option<&mut ResourceScope<'_>>,
) -> Result<AxmlDocument> {
    let mut doc = AxmlDocument::new(StringEncoding::Utf8);
    let scope = [Namespace {
        prefix: Some("android".to_string()),
        uri: ANDROID_NS.to_string(),
    }];
    let mut namespaces = Vec::new();
    declare(
        root,
        &scope,
        &mut namespaces,
        &mut doc,
        resources.as_deref().map(ResourceScope::table),
    )?;
    let declared: Vec<_> = namespaces
        .iter()
        .map(|decl| namespace_event(&mut doc, decl))
        .collect();
    doc.elements.extend(
        declared
            .iter()
            .map(|&(prefix, uri)| AxmlEvent::StartNamespace {
                metadata: NodeMetadata::default(),
                prefix,
                uri,
            }),
    );
    emit(root, &scope, &namespaces, &mut doc, &mut resources)?;
    doc.elements.extend(
        declared
            .into_iter()
            .map(|(prefix, uri)| AxmlEvent::EndNamespace {
                metadata: NodeMetadata::default(),
                prefix,
                uri,
            }),
    );
    Ok(doc)
}

/// The second pass: appends the element's events. A declaration that maps a
/// document namespace prefix to another uri wraps the element in its own
/// namespace events.
fn emit(
    element: &Element,
    parent: &[Namespace],
    namespaces: &[Namespace],
    doc: &mut AxmlDocument,
    resources: &mut Option<&mut ResourceScope<'_>>,
) -> Result<()> {
    let scope = scoped(parent, element);
    let shadows: Vec<_> = element
        .declarations()
        .filter(|decl| {
            namespaces
                .iter()
                .any(|global| global.prefix == decl.prefix && global.uri != decl.uri)
        })
        .map(|decl| namespace_event(doc, decl))
        .collect();
    doc.elements.extend(
        shadows
            .iter()
            .map(|&(prefix, uri)| AxmlEvent::StartNamespace {
                metadata: NodeMetadata::default(),
                prefix,
                uri,
            }),
    );
    let namespace = resolve_namespace(&element.name, &scope, NameUse::Element)?
        .map(|uri| doc.intern_string(&uri));
    let name = doc.intern_string(&element.name.local);
    let attributes = attributes(element, &scope, doc, resources)?;
    doc.elements.push(AxmlEvent::StartElement {
        metadata: NodeMetadata::default(),
        namespace,
        name,
        attributes,
    });
    for child in &element.children {
        match child {
            Node::Element(child) => emit(child, &scope, namespaces, doc, resources)?,
            Node::Text(text) if !text.trim().is_empty() => {
                let text = doc.intern_string(text);
                doc.elements.push(AxmlEvent::Text {
                    metadata: NodeMetadata::default(),
                    text,
                    value: ResValue::string(text),
                });
            }
            Node::Text(_) => {}
        }
    }
    doc.elements.push(AxmlEvent::EndElement {
        metadata: NodeMetadata::default(),
        namespace,
        name,
    });
    doc.elements.extend(
        shadows
            .into_iter()
            .rev()
            .map(|(prefix, uri)| AxmlEvent::EndNamespace {
                metadata: NodeMetadata::default(),
                prefix,
                uri,
            }),
    );
    Ok(())
}

/// The element's attributes with typed values, in aapt's order. Build-time
/// `tools:` hints are dropped.
fn attributes(
    element: &Element,
    scope: &[Namespace],
    doc: &mut AxmlDocument,
    resources: &mut Option<&mut ResourceScope<'_>>,
) -> Result<Vec<AxmlAttribute>> {
    let mut attributes = Vec::new();
    for (attr, text) in element.values() {
        let uri = resolve_namespace(attr, scope, NameUse::Attribute)?;
        if uri.as_deref() == Some(TOOLS_NS) {
            continue;
        }
        let res_id = attribute_resource_id(
            uri.as_deref(),
            &attr.local,
            resources.as_deref().map(ResourceScope::table),
        )?;
        let namespace = uri.as_deref().map(|uri| doc.intern_string(uri));
        let name = match res_id {
            Some(id) => doc.intern_attribute_name(&attr.local, id),
            None => doc.intern_string(&attr.local),
        };
        let value = match parse_value(text, res_id, resources.as_deref_mut())? {
            AttributeValue::Value(value) => value,
            AttributeValue::Text => ResValue::string(doc.intern_string(text)),
        };
        attributes.push(AxmlAttribute::new(namespace, name, value));
    }
    doc.sort_attributes(&mut attributes);
    Ok(attributes)
}
