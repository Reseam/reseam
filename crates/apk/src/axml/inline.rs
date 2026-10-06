// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::text::{self, Attribute, Element, Name, Namespace, Node};

use crate::error::{Result, invalid};

pub const AAPT_NS: &str = "http://schemas.android.com/aapt";

struct ExtractedInline {
    name: String,
    element: Element,
}

fn extract(
    root: Element,
    file_name: &str,
    res_type: &str,
) -> Result<(Element, Vec<ExtractedInline>)> {
    let mut extractor = Extractor {
        file_name,
        res_type,
        next: 0,
        resources: Vec::new(),
    };
    let root = extractor.element(root, &[])?;
    Ok((root, extractor.resources))
}

/// Compiled XML and the resource ID that points to its APK entry.
pub struct CompiledXmlResource {
    pub path: String,
    pub data: Vec<u8>,
    pub res_id: u32,
}

/// Extracts and compiles a resource file from one parsed XML tree. Inline
/// resources are compiled and registered before the files that reference them;
/// every returned file is ready to store in the APK. Registration uses the
/// local table, while references resolve through the lazy application scope.
pub fn compile_resource_file(
    text: &str,
    apk_path: &str,
    res_type: &str,
    name: &str,
    qualifiers: &str,
    scope: &mut crate::resources::ResourceScope<'_>,
) -> Result<Vec<CompiledXmlResource>> {
    let (root, inline) = extract(text::parse(text)?, name, res_type)?;
    let dir = apk_path.rsplit_once('/').map_or("", |(dir, _)| dir);
    let mut files = Vec::with_capacity(inline.len() + 1);
    for resource in inline {
        let path = format!("{dir}/{}.xml", resource.name);
        let data = super::compiler::build_element(&resource.element, Some(scope))?.serialize()?;
        let res_id =
            scope
                .table_mut()
                .add_file_resource(res_type, &resource.name, &path, qualifiers)?;
        files.push(CompiledXmlResource { path, data, res_id });
    }
    let data = super::compiler::build_element(&root, Some(scope))?.serialize()?;
    let res_id = scope
        .table_mut()
        .add_file_resource(res_type, name, apk_path, qualifiers)?;
    files.push(CompiledXmlResource {
        path: apk_path.to_string(),
        data,
        res_id,
    });
    Ok(files)
}

struct Extractor<'a> {
    file_name: &'a str,
    res_type: &'a str,
    resources: Vec<ExtractedInline>,
    next: usize,
}

impl Extractor<'_> {
    fn element(&mut self, mut element: Element, scope: &[Namespace]) -> Result<Element> {
        let mut scope = scope.to_vec();
        scope.extend(element.declarations().cloned());
        let mut children = Vec::with_capacity(element.children.len());
        for child in std::mem::take(&mut element.children) {
            match child {
                Node::Element(child)
                    if child.name.local == "attr"
                        && text::namespace(
                            scope.iter().chain(child.declarations()),
                            child.name.prefix.as_deref(),
                        ) == Some(AAPT_NS) =>
                {
                    let (attribute, reference) = self.inline(child, &scope)?;
                    if element
                        .values()
                        .any(|(name, _)| name.qualified() == attribute)
                    {
                        return Err(invalid(
                            "aapt:attr",
                            format!(
                                "<{}> sets {attribute} both inline and as an attribute",
                                element.name.qualified()
                            ),
                        ));
                    }
                    element.attributes.push(Attribute::Value {
                        name: Name::parse(&attribute),
                        text: reference,
                    });
                }
                Node::Element(child) => children.push(Node::Element(self.element(child, &scope)?)),
                text @ Node::Text(_) => children.push(text),
            }
        }
        element.children = children;
        Ok(element)
    }

    fn inline(&mut self, attr: Element, scope: &[Namespace]) -> Result<(String, String)> {
        let attribute = attr
            .values()
            .find(|(name, _)| name.prefix.is_none() && name.local == "name")
            .map(|(_, value)| value.to_string())
            .ok_or_else(|| invalid("aapt:attr", "an <aapt:attr> has no name"))?;
        let mut scope = scope.to_vec();
        scope.extend(attr.declarations().cloned());
        let mut elements = attr.children.into_iter().filter_map(|child| match child {
            Node::Element(element) => Some(element),
            Node::Text(_) => None,
        });
        let (Some(root), None) = (elements.next(), elements.next()) else {
            return Err(invalid(
                "aapt:attr",
                "an inline attribute must contain exactly one resource element",
            ));
        };
        // aapt assigns names before descending, but dependencies are emitted first.
        let name = format!("${}__{}", self.file_name, self.next);
        self.next += 1;
        let mut root = self.element(root, &scope)?;
        let mut declared = root
            .declarations()
            .map(|decl| decl.prefix.clone())
            .collect::<Vec<_>>();
        let mut inherited = Vec::new();
        for decl in scope.iter().rev() {
            if !declared.contains(&decl.prefix) {
                declared.push(decl.prefix.clone());
                inherited.push(Attribute::Namespace(decl.clone()));
            }
        }
        inherited.append(&mut root.attributes);
        root.attributes = inherited;
        self.resources.push(ExtractedInline {
            name: name.clone(),
            element: root,
        });
        Ok((attribute, format!("@{}/{name}", self.res_type)))
    }
}
