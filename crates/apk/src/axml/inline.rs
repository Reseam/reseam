// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `<aapt:attr>` inline resources. aapt compiles each one into a resource of
//! its own, of the enclosing file's type and named `$<file>__<n>`, and sets
//! the attribute it stands for to a reference to it.

use quick_xml::events::{BytesStart, Event};

use crate::error::{invalid, Result};

pub const AAPT_NS: &str = "http://schemas.android.com/aapt";

/// A resource that was written inline, as XML of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineResource {
    pub name: String,
    pub xml: String,
}

/// Moves every `<aapt:attr>` of `text` into an [`InlineResource`] named after
/// `file_name`, nested ones first, and returns `text` with each replaced by a
/// `@res_type/name` attribute on its parent. Text without inline resources
/// comes back unchanged.
pub fn extract_inline_resources(
    text: &str,
    file_name: &str,
    res_type: &str,
) -> Result<(String, Vec<InlineResource>)> {
    if !text.contains(AAPT_NS) {
        return Ok((text.to_string(), Vec::new()));
    }
    let root = parse(text)?;
    let mut extractor = Extractor {
        file_name,
        res_type,
        resources: Vec::new(),
    };
    let root = extractor.element(root, &[])?;
    Ok((serialize(&root, &[]), extractor.resources))
}

enum Node {
    Element(Element),
    Text(String),
}

struct Element {
    name: String,
    /// Attribute names and their values as written, still escaped.
    attributes: Vec<(String, String)>,
    children: Vec<Node>,
}

impl Element {
    fn declarations(&self) -> impl Iterator<Item = (&str, &str)> {
        self.attributes
            .iter()
            .filter_map(|(key, value)| Some((key.strip_prefix("xmlns:")?, value.as_str())))
    }
}

struct Extractor<'a> {
    file_name: &'a str,
    res_type: &'a str,
    resources: Vec<InlineResource>,
}

impl Extractor<'_> {
    /// `scope` holds the `xmlns` declarations of the ancestors, innermost last.
    fn element(&mut self, mut element: Element, scope: &[(String, String)]) -> Result<Element> {
        let mut scope = scope.to_vec();
        scope.extend(
            element
                .declarations()
                .map(|(prefix, uri)| (prefix.to_string(), uri.to_string())),
        );
        let mut children = Vec::with_capacity(element.children.len());
        for child in std::mem::take(&mut element.children) {
            match child {
                Node::Element(child) if is_inline(&child, &scope) => {
                    let (attribute, reference) = self.inline(child, &scope)?;
                    if element.attributes.iter().any(|(key, _)| *key == attribute) {
                        return Err(invalid(
                            "aapt:attr",
                            format!(
                                "<{}> sets {attribute} both inline and as an attribute",
                                element.name
                            ),
                        ));
                    }
                    element.attributes.push((attribute, reference));
                }
                Node::Element(child) => children.push(Node::Element(self.element(child, &scope)?)),
                text => children.push(text),
            }
        }
        element.children = children;
        Ok(element)
    }

    fn inline(&mut self, attr: Element, scope: &[(String, String)]) -> Result<(String, String)> {
        let attribute = attr
            .attributes
            .iter()
            .find(|(key, _)| key == "name")
            .map(|(_, value)| value.clone())
            .ok_or_else(|| invalid("aapt:attr", "an <aapt:attr> has no name"))?;
        let mut elements = attr.children.into_iter().filter_map(|child| match child {
            Node::Element(element) => Some(element),
            Node::Text(_) => None,
        });
        let (Some(root), None) = (elements.next(), elements.next()) else {
            return Err(invalid(
                "aapt:attr",
                format!("<aapt:attr name=\"{attribute}\"> must hold exactly one element"),
            ));
        };
        // Numbered before its nested resources, so a parent keeps aapt's lower index.
        let name = format!("${}__{}", self.file_name, self.resources.len());
        let index = self.resources.len();
        self.resources.push(InlineResource {
            name: name.clone(),
            xml: String::new(),
        });
        let root = self.element(root, scope)?;
        // The inline document stands alone, so it declares what its ancestors did.
        let xml = format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>{}",
            serialize(&root, scope)
        );
        // Nested resources were pushed after the placeholder; move this one behind
        // them so every resource follows the ones it references.
        let mut resource = self.resources.remove(index);
        resource.xml = xml;
        self.resources.push(resource);
        Ok((attribute, format!("@{}/{name}", self.res_type)))
    }
}

fn is_inline(element: &Element, scope: &[(String, String)]) -> bool {
    let Some((prefix, local)) = element.name.split_once(':') else {
        return false;
    };
    let declared = element
        .declarations()
        .map(|(p, uri)| (p.to_string(), uri.to_string()))
        .chain(scope.iter().rev().cloned())
        .find(|(p, _)| p == prefix)
        .map(|(_, uri)| uri);
    local == "attr" && declared.as_deref() == Some(AAPT_NS)
}

fn parse(text: &str) -> Result<Element> {
    let mut reader = quick_xml::Reader::from_str(text);
    let mut stack: Vec<Element> = Vec::new();
    let mut root = None;
    loop {
        let event = reader
            .read_event()
            .map_err(|e| invalid("aapt:attr", format!("XML parse error: {e}")))?;
        match event {
            Event::Eof => break,
            Event::Start(start) => stack.push(open(&start)?),
            Event::Empty(start) => close(open(&start)?, &mut stack, &mut root),
            Event::End(_) => {
                let element = stack
                    .pop()
                    .ok_or_else(|| invalid("aapt:attr", "unbalanced XML"))?;
                close(element, &mut stack, &mut root);
            }
            Event::Text(text) => {
                let raw = String::from_utf8_lossy(&text).into_owned();
                if let (false, Some(parent)) = (raw.trim().is_empty(), stack.last_mut()) {
                    parent.children.push(Node::Text(raw));
                }
            }
            Event::CData(data) => {
                if let Some(parent) = stack.last_mut() {
                    let raw = String::from_utf8_lossy(&data);
                    parent
                        .children
                        .push(Node::Text(format!("<![CDATA[{raw}]]>")));
                }
            }
            _ => {}
        }
    }
    root.ok_or_else(|| invalid("aapt:attr", "the XML has no root element"))
}

fn open(start: &BytesStart<'_>) -> Result<Element> {
    let attributes = start
        .attributes()
        .map(|attr| {
            let attr =
                attr.map_err(|e| invalid("aapt:attr", format!("invalid XML attribute: {e}")))?;
            Ok((
                String::from_utf8_lossy(attr.key.as_ref()).into_owned(),
                String::from_utf8_lossy(&attr.value).into_owned(),
            ))
        })
        .collect::<Result<_>>()?;
    Ok(Element {
        name: String::from_utf8_lossy(start.name().as_ref()).into_owned(),
        attributes,
        children: Vec::new(),
    })
}

fn close(element: Element, stack: &mut [Element], root: &mut Option<Element>) {
    match stack.last_mut() {
        Some(parent) => parent.children.push(Node::Element(element)),
        None => *root = Some(element),
    }
}

/// Writes `element`, declaring on it each namespace of `inherited` it does not
/// declare itself.
fn serialize(element: &Element, inherited: &[(String, String)]) -> String {
    let mut out = String::new();
    write_element(element, inherited, &mut out);
    out
}

fn write_element(element: &Element, inherited: &[(String, String)], out: &mut String) {
    out.push('<');
    out.push_str(&element.name);
    let own: Vec<&str> = element.declarations().map(|(prefix, _)| prefix).collect();
    let mut declared: Vec<&str> = Vec::new();
    for (prefix, uri) in inherited.iter().rev() {
        if own.contains(&prefix.as_str()) || declared.contains(&prefix.as_str()) {
            continue;
        }
        declared.push(prefix);
        out.push_str(&format!(" xmlns:{prefix}=\"{uri}\""));
    }
    for (key, value) in &element.attributes {
        out.push_str(&format!(" {key}=\"{value}\""));
    }
    if element.children.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    for child in &element.children {
        match child {
            Node::Element(child) => write_element(child, &[], out),
            Node::Text(text) => out.push_str(text),
        }
    }
    out.push_str(&format!("</{}>", element.name));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_resources_become_references_to_files_of_their_own() {
        let text = r#"<animated-vector xmlns:android="http://schemas.android.com/apk/res/android" xmlns:aapt="http://schemas.android.com/aapt">
    <aapt:attr name="android:drawable">
        <vector android:width="24dp"><path android:fillColor="@android:color/white"/></vector>
    </aapt:attr>
    <target android:name="icon">
        <aapt:attr name="android:animation">
            <set><objectAnimator android:propertyName="scaleX" android:valueTo="1.1"/></set>
        </aapt:attr>
    </target>
</animated-vector>"#;

        let (parent, inline) = extract_inline_resources(text, "icon", "drawable").unwrap();

        assert_eq!(
            inline.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
            ["$icon__0", "$icon__1"]
        );
        assert!(
            parent.contains(r#"android:drawable="@drawable/$icon__0""#),
            "{parent}"
        );
        assert!(
            parent.contains(
                r#"<target android:name="icon" android:animation="@drawable/$icon__1"/>"#
            ),
            "{parent}"
        );
        assert!(!parent.contains("aapt:attr"), "{parent}");
        assert!(
            inline[0].xml.contains(r#"<vector xmlns:aapt="http://schemas.android.com/aapt" xmlns:android="http://schemas.android.com/apk/res/android" android:width="24dp">"#),
            "{}",
            inline[0].xml
        );
        assert!(
            inline[1].xml.contains("<objectAnimator"),
            "{}",
            inline[1].xml
        );
    }

    #[test]
    fn a_nested_inline_resource_precedes_the_one_that_references_it() {
        let text = r#"<a xmlns:aapt="http://schemas.android.com/aapt" xmlns:android="http://schemas.android.com/apk/res/android"><aapt:attr name="android:drawable"><b><aapt:attr name="android:src"><c/></aapt:attr></b></aapt:attr></a>"#;

        let (parent, inline) = extract_inline_resources(text, "f", "drawable").unwrap();

        assert_eq!(
            inline.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
            ["$f__1", "$f__0"]
        );
        assert!(
            inline[1].xml.contains(r#"android:src="@drawable/$f__1""#),
            "{}",
            inline[1].xml
        );
        assert!(
            parent.contains(r#"android:drawable="@drawable/$f__0""#),
            "{parent}"
        );
    }

    #[test]
    fn text_without_inline_resources_is_unchanged() {
        let text = "<vector><path/></vector>";
        assert_eq!(
            extract_inline_resources(text, "f", "drawable").unwrap(),
            (text.to_string(), Vec::new())
        );
    }
}
