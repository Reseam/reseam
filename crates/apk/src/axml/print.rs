// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Binary XML written back as text, with resource ids resolved to names the
//! way the source XML wrote them.

use std::collections::HashMap;
use std::fmt::Write;

use super::android_attrs::{android_attr_value_name, android_res_name};
use super::{AxmlAttribute, AxmlDocument, AxmlEvent, element_end};
use crate::error::{Result, invalid};
use crate::res_value::ResValue;
use crate::resources::{ResourceName, ResourceTable};

const INDENT: &str = "    ";

impl AxmlDocument {
    /// The document as XML text. References to the app's resources are named
    /// through `table` when given, framework ones always; an id neither names
    /// prints as `@0x7f010001`.
    pub fn to_xml(&self, table: Option<&ResourceTable>) -> Result<String> {
        let mut out = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
        self.printer(table).print(self.events(), &mut out)?;
        Ok(out)
    }

    /// The element starting at event `start` and its children, as XML
    /// without the declaration; namespaces declared above it keep their prefixes.
    pub fn element_to_xml(&self, start: usize, table: Option<&ResourceTable>) -> Result<String> {
        let end = element_end(self.events(), start)
            .ok_or_else(|| invalid("binary XML", format!("event {start} starts no element")))?;
        let mut out = String::new();
        self.printer(table)
            .print(&self.events()[start..=end], &mut out)?;
        Ok(out)
    }

    fn printer<'a>(&'a self, table: Option<&'a ResourceTable>) -> Printer<'a> {
        Printer {
            document: self,
            table,
            prefixes: self.prefixes(),
        }
    }

    /// The prefix each namespace URI is declared with.
    fn prefixes(&self) -> HashMap<u32, String> {
        self.events()
            .iter()
            .filter_map(|event| match event {
                AxmlEvent::StartNamespace { prefix, uri, .. } => Some((
                    *uri,
                    prefix
                        .and_then(|prefix| self.string(prefix))
                        .map(std::borrow::Cow::into_owned)
                        .unwrap_or_default(),
                )),
                _ => None,
            })
            .collect()
    }
}

struct Printer<'a> {
    document: &'a AxmlDocument,
    table: Option<&'a ResourceTable>,
    prefixes: HashMap<u32, String>,
}

impl Printer<'_> {
    fn print(&self, events: &[AxmlEvent], out: &mut String) -> Result<()> {
        let mut depth = 0usize;
        let mut pending = Vec::new();
        let mut open = false;
        for event in events {
            match event {
                AxmlEvent::StartNamespace { prefix, uri, .. } => pending.push((*prefix, *uri)),
                AxmlEvent::StartElement {
                    namespace,
                    name,
                    attributes,
                    ..
                } => {
                    if open {
                        out.push_str(">\n");
                    }
                    out.push_str(&INDENT.repeat(depth));
                    out.push('<');
                    out.push_str(&self.qualified(*namespace, *name));
                    for (prefix, uri) in pending.drain(..) {
                        let prefix = prefix.and_then(|prefix| self.document.string(prefix));
                        let uri = self.document.string(uri).unwrap_or_default();
                        match prefix {
                            Some(prefix) => write!(out, " xmlns:{prefix}=\"{}\"", escape(&uri)),
                            None => write!(out, " xmlns=\"{}\"", escape(&uri)),
                        }
                        .expect("writing to a String");
                    }
                    for attribute in attributes {
                        write!(
                            out,
                            " {}=\"{}\"",
                            self.attribute_name(attribute),
                            escape(&self.attribute_value(attribute)?)
                        )
                        .expect("writing to a String");
                    }
                    open = true;
                    depth += 1;
                }
                AxmlEvent::Text { text, .. } => {
                    if open {
                        out.push_str(">\n");
                        open = false;
                    }
                    let text = self.document.string(*text).unwrap_or_default();
                    if !text.trim().is_empty() {
                        writeln!(out, "{}{}", INDENT.repeat(depth), escape(text.trim()))
                            .expect("writing to a String");
                    }
                }
                AxmlEvent::EndElement {
                    namespace, name, ..
                } => {
                    depth = depth.saturating_sub(1);
                    if open {
                        out.push_str(" />\n");
                        open = false;
                    } else {
                        writeln!(
                            out,
                            "{}</{}>",
                            INDENT.repeat(depth),
                            self.qualified(*namespace, *name)
                        )
                        .expect("writing to a String");
                    }
                }
                AxmlEvent::EndNamespace { .. }
                | AxmlEvent::Opaque(_)
                | AxmlEvent::StringPool
                | AxmlEvent::ResourceMap => {}
            }
        }
        Ok(())
    }

    fn qualified(&self, namespace: Option<u32>, name: u32) -> String {
        let local = self.document.string(name).unwrap_or_default();
        match namespace.and_then(|uri| self.prefixes.get(&uri)) {
            Some(prefix) if !prefix.is_empty() => format!("{prefix}:{local}"),
            _ => local.into_owned(),
        }
    }

    /// The attribute's name; an obfuscated build can leave the pool entry empty,
    /// and then the framework attribute its resource id names stands in.
    fn attribute_name(&self, attribute: &AxmlAttribute) -> String {
        let name = self.qualified(attribute.namespace(), attribute.name());
        if !name.is_empty() && !name.ends_with(':') {
            return name;
        }
        self.document
            .resource_id_for(attribute.name())
            .and_then(android_res_name)
            .map_or(name, |resource| format!("android:{}", resource.entry))
    }

    fn attribute_value(&self, attribute: &AxmlAttribute) -> Result<String> {
        let value = attribute.value();
        Ok(match value.kind {
            ResValue::STRING => self
                .document
                .attribute_string(attribute)
                .unwrap_or_default()
                .into_owned(),
            ResValue::REFERENCE if value.data != 0 => self.reference('@', value.data)?,
            ResValue::ATTRIBUTE => self.reference('?', value.data)?,
            ResValue::INT_DEC | ResValue::INT_HEX => self
                .document
                .resource_id_for(attribute.name())
                .and_then(|attr_id| android_attr_value_name(attr_id, value.data))
                .or_else(|| value.to_text())
                .unwrap_or_default(),
            _ => value
                .to_text()
                .unwrap_or_else(|| format!("0x{:08x}", value.data)),
        })
    }

    /// `@type/name` or `?attr/name`, framework resources prefixed `android:`.
    fn reference(&self, sigil: char, id: u32) -> Result<String> {
        let name: Option<ResourceName> = match id >> 24 {
            0x01 => android_res_name(id),
            _ => match self.table {
                Some(table) => table.resource_name(id)?,
                None => None,
            },
        };
        Ok(match name {
            Some(name) => name.reference(sigil),
            None => format!("{sigil}0x{id:08x}"),
        })
    }
}

fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\n' => escaped.push_str("&#10;"),
            c => escaped.push(c),
        }
    }
    escaped
}
