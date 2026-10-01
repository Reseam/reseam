// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::handles::checked;
use boltffi::export;
use reseam_apk::axml::{self, AttributeValue, AxmlAttribute, AxmlDocument};
use reseam_apk::{ResValue, ResourceScope, StringPool};

use super::xml_documents::{with_edit, with_read};
use super::xml_nodes::{ElementId, Tree};

fn split_name<'a>(document: &AxmlDocument, name: &'a str) -> (Option<u32>, &'a str) {
    name.split_once(':')
        .map_or((None, name), |(prefix, local)| {
            (document.declared_namespace(prefix), local)
        })
}

pub(super) fn attribute_text(
    document: &AxmlDocument,
    attributes: &[AxmlAttribute],
    name: &str,
) -> Option<String> {
    let (namespace, local) = split_name(document, name);
    if name.contains(':') && namespace.is_none() {
        return None;
    }
    let attribute = attributes.iter().find(|attribute| {
        attribute.namespace == namespace
            && document.string(attribute.name).as_deref() == Some(local)
    })?;
    Some(match attribute.value.kind {
        ResValue::STRING => document.attribute_string(attribute)?.into_owned(),
        ResValue::INT_DEC => (attribute.value.data as i32).to_string(),
        ResValue::INT_BOOLEAN => (attribute.value.data != 0).to_string(),
        ResValue::REFERENCE => format!("@0x{:08x}", attribute.value.data),
        ResValue::INT_HEX => format!("0x{:08x}", attribute.value.data),
        _ => attribute.value.data.to_string(),
    })
}

fn set_attribute(
    document: &mut AxmlDocument,
    attributes: &mut Vec<AxmlAttribute>,
    namespace: Option<u32>,
    name: u32,
    value: ResValue,
) {
    if let Some(attribute) = attributes
        .iter_mut()
        .find(|attribute| attribute.namespace == namespace && attribute.name == name)
    {
        attribute.set_value(value);
    } else {
        document.insert_attribute(attributes, AxmlAttribute::new(namespace, name, value));
    }
}

#[export]
pub fn xml_get_attribute(doc: u32, element: u32, name: String) -> Option<String> {
    let element = ElementId(element);
    checked(with_read(doc, |nodes, document| {
        Ok(attribute_text(
            document,
            nodes.attributes(document, element)?,
            &name,
        ))
    }))
}

fn set_attribute_value(
    doc: u32,
    element: ElementId,
    name: &str,
    value: impl FnOnce(
        &mut StringPool,
        Option<u32>,
        Option<&mut ResourceScope<'_>>,
    ) -> Result<ResValue, String>,
) -> Result<(), String> {
    with_edit(doc, |nodes, document, ctx| {
        let position = nodes.position(element)?;
        ctx.apk_mut()
            .with_resource_scope(0, |scope| {
                nodes.edit_attributes(document, element, |document, attributes| {
                    let index = if position.tree == Tree::Document {
                        position.index
                    } else {
                        document.root().unwrap_or(0)
                    };
                    let (namespace, name) = document
                        .bind_attribute_name_at(
                            index,
                            name,
                            scope.as_deref().map(ResourceScope::table),
                        )
                        .map_err(|e| e.to_string())?;
                    let attribute = namespace.and_then(|_| document.resource_id_for(name));
                    let value = value(document.string_pool_mut(), attribute, scope)?;
                    set_attribute(document, attributes, namespace, name, value);
                    Ok(())
                })
            })
            .map_err(|e| e.to_string())?
    })
}

#[export]
pub fn xml_set_attribute(
    doc: u32,
    element: u32,
    name: String,
    value: String,
) -> Result<(), String> {
    let element = ElementId(element);
    set_attribute_value(doc, element, &name, |pool, attribute, scope| {
        let parsed = match attribute {
            Some(id) => axml::parse_attribute_value(&value, id, scope),
            None => axml::infer_value(&value, scope),
        }
        .map_err(|e| format!("attribute {name}: {e}"))?;
        match parsed {
            AttributeValue::Value(value) => Ok(value),
            AttributeValue::Text => pool
                .intern(&value)
                .map(ResValue::string)
                .map_err(|e| e.to_string()),
        }
    })
}

#[export]
pub fn xml_set_attribute_ref(
    doc: u32,
    element: u32,
    name: String,
    resource: u32,
) -> Result<(), String> {
    let element = ElementId(element);
    set_attribute_value(doc, element, &name, |_, _, _| {
        Ok(ResValue::reference(resource))
    })
}

#[export]
pub fn xml_remove_attribute(doc: u32, element: u32, name: String) {
    let element = ElementId(element);
    checked(with_edit(doc, |nodes, document, _| {
        nodes.edit_attributes(document, element, |document, attributes| {
            let (namespace, local) = split_name(document, &name);
            if !name.contains(':') || namespace.is_some() {
                attributes.retain(|attribute| {
                    !(attribute.namespace == namespace
                        && document.string(attribute.name).as_deref() == Some(local))
                });
            }
            Ok(())
        })
    }));
}
