// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Compiles XML text into a binary document. A prefixed attribute resolves
//! through the `xmlns` declarations the text carries: the framework table for
//! `http://schemas.android.com/apk/res/android`, the app's own `attr` entries
//! for every other namespace.

use quick_xml::events::{BytesStart, Event};

use super::AAPT_NS;
use super::{
    android_attr_symbol, android_res_id, AxmlAttribute, AxmlDocument, AxmlEvent, ANDROID_NS,
    TOOLS_NS,
};
use crate::error::{invalid, Result};
use crate::resources::ResourceTable;
use crate::value::ResValue;

pub fn is_compiled_axml(data: &[u8]) -> bool {
    data.starts_with(&[0x03, 0x00, 0x08, 0x00])
}

/// `resources` resolves `@type/name` references, creates `@+id/name` entries
/// and supplies the ids of the app's own attributes; without it those
/// attributes stay plain strings.
pub fn compile_xml(text: &str, resources: Option<&mut ResourceTable>) -> Result<Vec<u8>> {
    build_document(text, resources)?.serialize()
}

pub fn build_document(text: &str, resources: Option<&mut ResourceTable>) -> Result<AxmlDocument> {
    let mut compiler = Compiler {
        doc: AxmlDocument::new(true),
        resources,
        namespaces: Vec::new(),
    };
    walk(text, |node| compiler.declare(node))?;
    // Names carrying a resource id must occupy the first pool indices so they
    // line up with the id table, so they are bound before any other string.
    walk(text, |node| compiler.bind(node))?;
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

fn local_or_qualified(element: &BytesStart<'_>) -> String {
    String::from_utf8_lossy(element.name().as_ref()).into_owned()
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

fn android_resource(type_name: &str, name: &str) -> Result<u32> {
    android_res_id(type_name, name).ok_or_else(|| {
        invalid(
            "axml compiler",
            format!("android:{type_name}/{name} is not a public framework resource"),
        )
    })
}

struct Compiler<'r> {
    doc: AxmlDocument,
    resources: Option<&'r mut ResourceTable>,
    namespaces: Vec<(String, String)>,
}

/// `prefix:local` for an attribute that belongs to a namespace.
fn qualified(key: &str) -> Option<(&str, &str)> {
    key.split_once(':').filter(|(prefix, _)| *prefix != "xmlns")
}

impl Compiler<'_> {
    /// Collects the document's `xmlns` declarations. An `android:` attribute in
    /// a fragment that declares nothing still means the framework namespace.
    fn declare(&mut self, node: Node<'_>) -> Result<()> {
        let Node::Start(element, _) = node else {
            return Ok(());
        };
        for (key, value) in element_attributes(&element)? {
            match key.strip_prefix("xmlns:") {
                Some(prefix) if !self.declares(prefix) => {
                    self.namespaces.push((prefix.to_string(), value))
                }
                Some(_) => {}
                None => {
                    if qualified(&key)
                        .is_some_and(|(prefix, _)| prefix == "android" && !self.declares("android"))
                    {
                        self.namespaces
                            .push(("android".to_string(), ANDROID_NS.to_string()));
                    }
                }
            }
        }
        Ok(())
    }

    fn declares(&self, prefix: &str) -> bool {
        self.namespaces.iter().any(|(name, _)| name == prefix)
    }

    fn namespace_uri(&self, prefix: &str) -> Option<&str> {
        self.namespaces
            .iter()
            .find(|(name, _)| name == prefix)
            .map(|(_, uri)| uri.as_str())
    }

    /// Interns every prefixed attribute name against the resource id the
    /// inflater resolves it by. A name that resolves to none fails the compile,
    /// since a compiled attribute means whatever its id says and no more.
    fn bind(&mut self, node: Node<'_>) -> Result<()> {
        let Node::Start(element, _) = node else {
            return Ok(());
        };
        if let Some((prefix, _)) = qualified(&local_or_qualified(&element)) {
            if self.namespace_uri(prefix) == Some(AAPT_NS) {
                return Err(invalid(
                    "axml compiler",
                    "<aapt:attr> is an inline resource; add the file with ResourceScope.addFile, which compiles it into a resource of its own",
                ));
            }
        }
        for (key, _) in element_attributes(&element)? {
            let Some((prefix, local)) = qualified(&key) else {
                continue;
            };
            let uri = self.namespace_uri(prefix).ok_or_else(|| {
                invalid(
                    "axml compiler",
                    format!("attribute {key}: the document declares no xmlns:{prefix}"),
                )
            })?;
            if uri == TOOLS_NS {
                continue;
            }
            let res_id = if uri == ANDROID_NS {
                android_resource("attr", local)?
            } else {
                self.resources
                    .as_deref()
                    .and_then(|table| table.find_resource_id("attr", local))
                    .ok_or_else(|| {
                        invalid(
                            "axml compiler",
                            format!(
                                "attribute {key}: the app's resource table has no attr/{local}, and an attribute without a resource id is ignored by the inflater"
                            ),
                        )
                    })?
            };
            let name = self.doc.intern_string(local);
            match self.doc.resource_id_for(name) {
                Some(bound) if bound != res_id => {
                    return Err(invalid(
                        "axml compiler",
                        format!("attribute {key}: {local} is already bound to {bound:#010x}"),
                    ))
                }
                Some(_) => {}
                None => self.doc.bind_resource_id(name, res_id),
            }
        }
        Ok(())
    }

    fn namespace_indices(&mut self) -> Vec<(u32, u32)> {
        let declared = self.namespaces.clone();
        declared
            .iter()
            .map(|(prefix, uri)| (self.doc.intern_string(prefix), self.doc.intern_string(uri)))
            .collect()
    }

    fn open_namespaces(&mut self) {
        for (prefix, uri) in self.namespace_indices() {
            self.doc.elements.push(AxmlEvent::StartNamespace {
                prefix: Some(prefix),
                uri,
            });
        }
    }

    fn close_namespaces(&mut self) {
        for (prefix, uri) in self.namespace_indices() {
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
                    let (namespace, local) = match qualified(&key) {
                        Some((prefix, local)) => {
                            let uri = self.namespace_uri(prefix).unwrap_or_default().to_string();
                            if uri == TOOLS_NS {
                                continue;
                            }
                            (Some(self.doc.intern_string(&uri)), local)
                        }
                        None => (None, key.as_str()),
                    };
                    let name = self.doc.intern_string(local);
                    let attr = namespace.and_then(|_| self.doc.resource_id_for(name));
                    let value = self.value(&value, attr)?;
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

    fn value(&mut self, text: &str, attr: Option<u32>) -> Result<ResValue> {
        Ok(
            match parse_attribute_value(text, attr, self.resources.as_deref_mut())? {
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

/// Parses a value of attribute `attr` the way aapt does: the enum or flag
/// names `attr` defines, booleans, layout keywords, colors, dimensions,
/// numbers, then `?attr` and `@type/name` references resolved against
/// `resources`. Anything else is text.
pub fn parse_attribute_value(
    text: &str,
    attr: Option<u32>,
    resources: Option<&mut ResourceTable>,
) -> Result<AttributeValue> {
    if let Some(value) = attr.and_then(|attr| attribute_symbols(attr, text, resources.as_deref())) {
        return Ok(AttributeValue::Value(value));
    }
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

/// One enum name, or flag names joined by `|`, as the integer aapt compiles
/// them to: decimal for an enum, hexadecimal for flags.
pub fn attribute_symbols(
    attr: u32,
    text: &str,
    resources: Option<&ResourceTable>,
) -> Option<ResValue> {
    let symbols = text
        .split('|')
        .map(str::trim)
        .map(|name| android_attr_symbol(attr, name).or_else(|| resources?.attr_symbol(attr, name)))
        .collect::<Option<Vec<_>>>()?;
    match symbols.as_slice() {
        [symbol] if !symbol.flags => Some(ResValue::int(symbol.value as i32)),
        _ if symbols.iter().all(|symbol| symbol.flags) => Some(ResValue::hex(
            symbols.iter().fold(0, |mask, symbol| mask | symbol.value),
        )),
        _ => None,
    }
}

/// A bare id, as `XmlElement.get` and `ResourceScope.getArray` render one:
/// `0x7f140a59`, or `ref/0x7f140a59` for the form manifest attributes use.
fn hex_ref(text: &str) -> Option<u32> {
    let hex = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("ref/0x"))?;
    u32::from_str_radix(hex, 16).ok()
}

/// `?android:attr/name`, `?attr/name` or `?0x...`.
fn attribute_ref(text: &str, resources: Option<&ResourceTable>) -> Result<Option<u32>> {
    if let Some(id) = hex_ref(text) {
        return Ok(Some(id));
    }
    if let Some(name) = text.strip_prefix("android:attr/") {
        return android_resource("attr", name).map(Some);
    }
    let name = text.strip_prefix("attr/").unwrap_or(text);
    Ok(resources.and_then(|res| res.find_resource_id("attr", name)))
}

/// `[+][namespace:]type/name` or `0x...`; `+id/name` creates the id entry.
fn resource_ref(text: &str, resources: Option<&mut ResourceTable>) -> Result<Option<u32>> {
    if let Some(id) = hex_ref(text) {
        return Ok(Some(id));
    }
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
        (Some("android"), _) => Some(android_resource(type_name, entry)?),
        (Some(_), _) | (None, None) => None,
        (None, Some(res)) if create => res.ensure_id(entry),
        (None, Some(res)) => res.find_resource_id(type_name, entry),
    })
}
