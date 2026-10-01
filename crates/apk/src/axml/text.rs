// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use quick_xml::events::{BytesStart, Event};

use crate::error::{Result, invalid};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Name {
    pub prefix: Option<String>,
    pub local: String,
}

impl Name {
    pub fn parse(text: &str) -> Self {
        match text.split_once(':') {
            Some((prefix, local)) => Self {
                prefix: Some(prefix.to_string()),
                local: local.to_string(),
            },
            None => Self {
                prefix: None,
                local: text.to_string(),
            },
        }
    }

    pub fn qualified(&self) -> String {
        self.prefix.as_ref().map_or_else(
            || self.local.clone(),
            |prefix| format!("{prefix}:{}", self.local),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Namespace {
    pub prefix: Option<String>,
    pub uri: String,
}

#[derive(Debug, Clone)]
pub(super) enum Attribute {
    Namespace(Namespace),
    Value { name: Name, text: String },
}

pub(super) enum Node {
    Element(Element),
    Text(String),
}

pub(super) struct Element {
    pub name: Name,
    pub attributes: Vec<Attribute>,
    pub children: Vec<Node>,
}

impl Element {
    pub fn declarations(&self) -> impl DoubleEndedIterator<Item = &Namespace> {
        self.attributes.iter().filter_map(|attr| match attr {
            Attribute::Namespace(namespace) => Some(namespace),
            Attribute::Value { .. } => None,
        })
    }

    pub fn values(&self) -> impl Iterator<Item = (&Name, &str)> {
        self.attributes.iter().filter_map(|attr| match attr {
            Attribute::Value { name, text } => Some((name, text.as_str())),
            Attribute::Namespace(_) => None,
        })
    }
}

pub(super) fn namespace<'a>(
    scope: impl DoubleEndedIterator<Item = &'a Namespace>,
    prefix: Option<&str>,
) -> Option<&'a str> {
    scope
        .rev()
        .find(|decl| decl.prefix.as_deref() == prefix)
        .map(|decl| decl.uri.as_str())
}

pub(super) fn parse(text: &str) -> Result<Element> {
    let mut reader = quick_xml::Reader::from_str(text);
    let mut stack = Vec::new();
    let mut root = None;
    loop {
        let event = reader.read_event().map_err(|error| {
            invalid(
                "XML text",
                format!("at {}: {error}", reader.error_position()),
            )
        })?;
        match event {
            Event::Eof => break,
            Event::Start(start) => stack.push(open(&start)?),
            Event::Empty(start) => close(open(&start)?, &mut stack, &mut root)?,
            Event::End(_) => {
                let element = stack
                    .pop()
                    .ok_or_else(|| invalid("XML text", "unbalanced closing element"))?;
                close(element, &mut stack, &mut root)?;
            }
            Event::Text(text) => append_text(&mut stack, text.as_ref())?,
            Event::CData(text) => append_decoded(&mut stack, text.as_ref())?,
            Event::GeneralRef(reference) => {
                append_text(&mut stack, &format!("&{};", reference.as_ref()))?;
            }
            Event::DocType(_) => {
                return Err(invalid("XML text", "DTD declarations are unsupported"));
            }
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Err(invalid("XML text", "unterminated element"));
    }
    root.ok_or_else(|| invalid("XML text", "document has no root element"))
}

fn append_text(stack: &mut [Element], escaped: &str) -> Result<()> {
    let text = quick_xml::escape::unescape(escaped)
        .map_err(|error| invalid("XML text", error.to_string()))?;
    append_decoded(stack, &text)
}

fn append_decoded(stack: &mut [Element], text: &str) -> Result<()> {
    let Some(parent) = stack.last_mut() else {
        return if text.trim().is_empty() {
            Ok(())
        } else {
            Err(invalid("XML text", "text outside the root element"))
        };
    };
    match parent.children.last_mut() {
        Some(Node::Text(previous)) => previous.push_str(text),
        _ => parent.children.push(Node::Text(text.to_string())),
    }
    Ok(())
}

fn open(start: &BytesStart<'_>) -> Result<Element> {
    let attributes = start
        .attributes()
        .map(|attr| {
            let attr = attr.map_err(|error| invalid("XML attribute", error.to_string()))?;
            let text = quick_xml::escape::unescape(&attr.value)
                .map_err(|error| invalid("XML attribute", error.to_string()))?
                .into_owned();
            let name = Name::parse(attr.key.as_ref());
            Ok(match (name.prefix.as_deref(), name.local.as_str()) {
                (None, "xmlns") => Attribute::Namespace(Namespace {
                    prefix: None,
                    uri: text,
                }),
                (Some("xmlns"), _) => Attribute::Namespace(Namespace {
                    prefix: Some(name.local),
                    uri: text,
                }),
                _ => Attribute::Value { name, text },
            })
        })
        .collect::<Result<_>>()?;
    Ok(Element {
        name: Name::parse(start.name().as_ref()),
        attributes,
        children: Vec::new(),
    })
}

fn close(element: Element, stack: &mut [Element], root: &mut Option<Element>) -> Result<()> {
    match stack.last_mut() {
        Some(parent) => parent.children.push(Node::Element(element)),
        None if root.is_none() => *root = Some(element),
        None => {
            return Err(invalid(
                "XML text",
                "document has more than one root element",
            ));
        }
    }
    Ok(())
}
