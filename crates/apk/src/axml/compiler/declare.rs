// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::super::bind::attribute_resource_id;
use super::super::text::{self, Attribute, Element, Name, Namespace, Node};
use super::super::{AAPT_NS, AxmlDocument, TOOLS_NS};
use crate::error::{Result, invalid};
use crate::resources::ResourceTable;

/// The first pass: validates names, records the first declaration of each
/// prefix as a document namespace, and binds attribute names to their ids.
pub(super) fn declare(
    element: &Element,
    parent: &[Namespace],
    namespaces: &mut Vec<Namespace>,
    doc: &mut AxmlDocument,
    table: Option<&ResourceTable>,
) -> Result<()> {
    let scope = scoped(parent, element);
    for attr in &element.attributes {
        let decl = match attr {
            Attribute::Namespace(decl) => Some(decl.clone()),
            Attribute::Value { name, .. } if name.prefix.as_deref() == Some("android") => {
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
    for (name, _) in element.values() {
        let uri = resolve_namespace(name, &scope, NameUse::Attribute)?;
        if uri.as_deref() == Some(TOOLS_NS) {
            continue;
        }
        if let Some(id) = attribute_resource_id(uri.as_deref(), &name.local, table)? {
            doc.intern_attribute_name(&name.local, id);
        }
    }
    for child in &element.children {
        if let Node::Element(child) = child {
            declare(child, &scope, namespaces, doc, table)?;
        }
    }
    Ok(())
}

/// The namespaces in scope inside `element`.
pub(super) fn scoped(parent: &[Namespace], element: &Element) -> Vec<Namespace> {
    parent
        .iter()
        .chain(element.declarations())
        .cloned()
        .collect()
}

pub(super) fn namespace_event(doc: &mut AxmlDocument, decl: &Namespace) -> (Option<u32>, u32) {
    (
        decl.prefix
            .as_deref()
            .map(|prefix| doc.intern_string(prefix)),
        doc.intern_string(&decl.uri),
    )
}

#[derive(Clone, Copy)]
pub(super) enum NameUse {
    Element,
    Attribute,
}

pub(super) fn resolve_namespace(
    name: &Name,
    scope: &[Namespace],
    use_of: NameUse,
) -> Result<Option<String>> {
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
