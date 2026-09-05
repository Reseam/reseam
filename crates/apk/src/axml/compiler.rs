// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Compiles XML text into a binary document. Only the `android:` and `app:`
//! prefixes are understood; other prefixes stay part of the attribute name.

use quick_xml::events::{BytesStart, Event};

use super::{android_attr_res_id, AxmlAttribute, AxmlDocument, AxmlEvent, ANDROID_NS, APP_NS};
use crate::error::{invalid, Result};
use crate::resources::ResourceTable;
use crate::value::ResValue;

pub fn is_compiled_axml(data: &[u8]) -> bool {
    data.starts_with(&[0x03, 0x00, 0x08, 0x00])
}

/// `resources` resolves `@type/name` references and creates `@+id/name`
/// entries; without it those attributes stay plain strings.
pub fn compile_xml(text: &str, resources: Option<&mut ResourceTable>) -> Result<Vec<u8>> {
    build_document(text, resources)?.serialize()
}

pub fn build_document(text: &str, resources: Option<&mut ResourceTable>) -> Result<AxmlDocument> {
    let mut compiler = Compiler {
        doc: AxmlDocument::new(true),
        resources,
        android: false,
        app: false,
    };
    walk(text, |node| compiler.scan(node))?;
    compiler.open_namespaces();
    walk(text, |node| compiler.emit(node))?;
    compiler.close_namespaces();
    Ok(compiler.doc)
}

enum Node<'a> {
    Start(BytesStart<'a>, bool),
    End(String),
}

fn walk(text: &str, mut visit: impl FnMut(Node<'_>) -> Result<()>) -> Result<()> {
    let mut reader = quick_xml::Reader::from_str(text);
    reader.config_mut().trim_text(true);
    loop {
        let event = reader
            .read_event()
            .map_err(|e| invalid("axml compiler", format!("XML parse error: {e}")))?;
        match event {
            Event::Eof => return Ok(()),
            Event::Start(element) => visit(Node::Start(element, false))?,
            Event::Empty(element) => visit(Node::Start(element, true))?,
            Event::End(element) => visit(Node::End(local_name(element.name().as_ref())))?,
            _ => {}
        }
    }
}

fn local_name(qualified: &[u8]) -> String {
    let name = std::str::from_utf8(qualified).unwrap_or("");
    name.rsplit(':').next().unwrap_or(name).to_string()
}

fn element_attributes(element: &BytesStart<'_>) -> Result<Vec<(String, String)>> {
    element
        .attributes()
        .map(|attr| {
            let attr =
                attr.map_err(|e| invalid("axml compiler", format!("invalid XML attribute: {e}")))?;
            let key = std::str::from_utf8(attr.key.as_ref()).map_err(|e| {
                invalid(
                    "axml compiler",
                    format!("invalid UTF-8 in attribute key: {e}"),
                )
            })?;
            let value = attr.unescape_value().map_err(|e| {
                invalid("axml compiler", format!("invalid XML attribute value: {e}"))
            })?;
            Ok((key.to_string(), value.into_owned()))
        })
        .collect()
}

fn android_attr(name: &str) -> Result<u32> {
    android_attr_res_id(name).ok_or_else(|| {
        invalid(
            "axml compiler",
            format!("unknown android attribute `{name}`"),
        )
    })
}

struct Compiler<'r> {
    doc: AxmlDocument,
    resources: Option<&'r mut ResourceTable>,
    android: bool,
    app: bool,
}

impl Compiler<'_> {
    /// Attribute names with framework ids must occupy the first pool indices
    /// so they line up with the resource id table, so they are interned
    /// before anything else.
    fn scan(&mut self, node: Node<'_>) -> Result<()> {
        let Node::Start(element, _) = node else {
            return Ok(());
        };
        for (key, _) in element_attributes(&element)? {
            if let Some(local) = key.strip_prefix("android:") {
                let res_id = android_attr(local)?;
                let name = self.doc.intern_string(local);
                self.doc.bind_resource_id(name, res_id);
                self.android = true;
            } else if key == "xmlns:android" {
                self.android = true;
            } else if key == "xmlns:app" || key.starts_with("app:") {
                self.app = true;
            }
        }
        Ok(())
    }

    fn namespaces(&mut self) -> Vec<(u32, u32)> {
        let mut out = Vec::new();
        if self.android {
            out.push((
                self.doc.intern_string("android"),
                self.doc.intern_string(ANDROID_NS),
            ));
        }
        if self.app {
            out.push((
                self.doc.intern_string("app"),
                self.doc.intern_string(APP_NS),
            ));
        }
        out
    }

    fn open_namespaces(&mut self) {
        for (prefix, uri) in self.namespaces() {
            self.doc.elements.push(AxmlEvent::StartNamespace {
                prefix: Some(prefix),
                uri,
            });
        }
    }

    fn close_namespaces(&mut self) {
        for (prefix, uri) in self.namespaces() {
            self.doc.elements.push(AxmlEvent::EndNamespace {
                prefix: Some(prefix),
                uri,
            });
        }
    }

    fn emit(&mut self, node: Node<'_>) -> Result<()> {
        match node {
            Node::Start(element, empty) => {
                let name = self.doc.intern_string(&local_name(element.name().as_ref()));
                let mut attributes = Vec::new();
                for (key, value) in element_attributes(&element)? {
                    if key == "xmlns" || key.starts_with("xmlns:") {
                        continue;
                    }
                    let (namespace, local) = if let Some(local) = key.strip_prefix("android:") {
                        (Some(self.doc.intern_string(ANDROID_NS)), local)
                    } else if let Some(local) = key.strip_prefix("app:") {
                        (Some(self.doc.intern_string(APP_NS)), local)
                    } else {
                        (None, key.as_str())
                    };
                    let name = self.doc.intern_string(local);
                    let value = self.value(&value)?;
                    attributes.push(AxmlAttribute::new(namespace, name, value));
                }
                attributes
                    .sort_by_key(|attr| self.doc.resource_id_for(attr.name).unwrap_or(u32::MAX));
                self.doc.elements.push(AxmlEvent::StartElement {
                    namespace: None,
                    name,
                    attributes,
                });
                if empty {
                    self.doc.elements.push(AxmlEvent::EndElement {
                        namespace: None,
                        name,
                    });
                }
            }
            Node::End(name) => {
                let name = self.doc.intern_string(&name);
                self.doc.elements.push(AxmlEvent::EndElement {
                    namespace: None,
                    name,
                });
            }
        }
        Ok(())
    }

    fn value(&mut self, text: &str) -> Result<ResValue> {
        Ok(
            match parse_attribute_value(text, self.resources.as_deref_mut())? {
                AttributeValue::Value(value) => value,
                AttributeValue::Text => ResValue::string(self.doc.intern_string(text)),
            },
        )
    }
}

/// What an attribute's text means once literals and references are parsed.
pub enum AttributeValue {
    Value(ResValue),
    /// Plain text the caller interns into its own string pool.
    Text,
}

/// Parses an attribute value the way aapt does: booleans, layout keywords,
/// colors, dimensions, numbers, then `?attr` and `@type/name` references
/// resolved against `resources`. Anything else is text.
pub fn parse_attribute_value(
    text: &str,
    resources: Option<&mut ResourceTable>,
) -> Result<AttributeValue> {
    let literal = match text {
        "true" => Some(ResValue::boolean(true)),
        "false" => Some(ResValue::boolean(false)),
        "match_parent" | "fill_parent" => Some(ResValue::int(-1)),
        "wrap_content" => Some(ResValue::int(-2)),
        "@null" | "@empty" => Some(ResValue::reference(0)),
        _ => None,
    }
    .or_else(|| ResValue::parse_color(text))
    .or_else(|| ResValue::parse_dimension(text))
    .or_else(|| {
        text.strip_prefix("0x")
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
            .map(ResValue::hex)
    })
    .or_else(|| text.parse::<i32>().ok().map(ResValue::int))
    .or_else(|| text.parse::<f32>().ok().map(ResValue::float));
    if let Some(value) = literal {
        return Ok(AttributeValue::Value(value));
    }
    if let Some(id) = text
        .strip_prefix('?')
        .map(|r| attribute_ref(r, resources.as_deref()))
        .transpose()?
        .flatten()
    {
        return Ok(AttributeValue::Value(ResValue::attribute(id)));
    }
    if let Some(id) = text
        .strip_prefix('@')
        .map(|r| resource_ref(r, resources))
        .transpose()?
        .flatten()
    {
        return Ok(AttributeValue::Value(ResValue::reference(id)));
    }
    Ok(AttributeValue::Text)
}

/// `?android:attr/name` or `?attr/name`.
fn attribute_ref(text: &str, resources: Option<&ResourceTable>) -> Result<Option<u32>> {
    if let Some(name) = text.strip_prefix("android:attr/") {
        return android_attr(name).map(Some);
    }
    let name = text.strip_prefix("attr/").unwrap_or(text);
    Ok(resources.and_then(|res| res.find_resource_id("attr", name)))
}

/// `[+][namespace:]type/name`; `+id/name` creates the id entry.
fn resource_ref(text: &str, resources: Option<&mut ResourceTable>) -> Result<Option<u32>> {
    let create = text.starts_with("+id/");
    let text = text.strip_prefix('+').unwrap_or(text);
    let Some((type_part, entry)) = text.split_once('/') else {
        return Ok(None);
    };
    let (namespace, type_name) = match type_part.split_once(':') {
        Some((namespace, type_name)) => (Some(namespace), type_name),
        None => (None, type_part),
    };
    if type_name.is_empty() || entry.is_empty() {
        return Ok(None);
    }
    Ok(match (namespace, resources) {
        (Some("android"), _) if type_name == "attr" => Some(android_attr(entry)?),
        (Some(_), _) | (None, None) => None,
        (None, Some(res)) if create => res.ensure_id(entry),
        (None, Some(res)) => res.find_resource_id(type_name, entry),
    })
}
