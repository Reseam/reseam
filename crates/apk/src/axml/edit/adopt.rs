// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{
    AxmlAttribute, AxmlDocument, AxmlEvent, NodeMetadata, ResValue, ResourceTable, Result, invalid,
};

impl AxmlDocument {
    /// A deep copy of `events`, a subtree of `source`, in this document's own
    /// strings and namespaces. Every attribute is re-bound to the id this
    /// document resolves it by, so an adopted attribute carries what the
    /// inflater reads or the adoption fails.
    pub fn adopt(
        &mut self,
        source: &AxmlDocument,
        events: &[AxmlEvent],
        resources: Option<&ResourceTable>,
    ) -> Result<Vec<AxmlEvent>> {
        let mut declarations = Vec::new();
        events
            .iter()
            .map(|event| match event {
                AxmlEvent::StartElement {
                    namespace,
                    name,
                    attributes,
                    ..
                } => {
                    let namespace = self.adopt_namespace(source, *namespace, &declarations)?;
                    let name = self.adopt_string(source, *name)?;
                    let mut adopted = attributes
                        .iter()
                        .map(|attr| self.adopt_attribute(source, attr, resources, &declarations))
                        .collect::<Result<Vec<_>>>()?;
                    self.sort_attributes(&mut adopted);
                    Ok(AxmlEvent::StartElement {
                        metadata: NodeMetadata::default(),
                        namespace,
                        name,
                        attributes: adopted,
                    })
                }
                AxmlEvent::EndElement {
                    namespace, name, ..
                } => Ok(AxmlEvent::EndElement {
                    metadata: NodeMetadata::default(),
                    namespace: self.adopt_namespace(source, *namespace, &declarations)?,
                    name: self.adopt_string(source, *name)?,
                }),
                AxmlEvent::Text { text, value, .. } => {
                    let text = self.adopt_string(source, *text)?;
                    let value = match value.string_index() {
                        Some(index) => ResValue::string(self.adopt_string(source, index)?),
                        None => *value,
                    };
                    Ok(AxmlEvent::Text {
                        metadata: NodeMetadata::default(),
                        text,
                        value,
                    })
                }
                AxmlEvent::Opaque(_) | AxmlEvent::StringPool | AxmlEvent::ResourceMap => {
                    Err(invalid(
                        "axml adopt",
                        "cannot remap an opaque chunk or document pool",
                    ))
                }
                AxmlEvent::StartNamespace { prefix, uri, .. } => {
                    let prefix = prefix
                        .map(|index| self.adopt_string(source, index))
                        .transpose()?;
                    let uri = self.adopt_string(source, *uri)?;
                    declarations.push(uri);
                    Ok(AxmlEvent::StartNamespace {
                        metadata: NodeMetadata::default(),
                        prefix,
                        uri,
                    })
                }
                AxmlEvent::EndNamespace { prefix, uri, .. } => {
                    let prefix = prefix
                        .map(|index| self.adopt_string(source, index))
                        .transpose()?;
                    let uri = self.adopt_string(source, *uri)?;
                    if let Some(index) = declarations.iter().rposition(|&declared| declared == uri)
                    {
                        declarations.remove(index);
                    }
                    Ok(AxmlEvent::EndNamespace {
                        metadata: NodeMetadata::default(),
                        prefix,
                        uri,
                    })
                }
            })
            .collect()
    }

    fn adopt_string(&mut self, source: &AxmlDocument, index: u32) -> Result<u32> {
        let value = Self::source_string(source, index)?;
        Ok(self.intern_string(&value))
    }

    fn source_string(source: &AxmlDocument, index: u32) -> Result<std::borrow::Cow<'_, str>> {
        source.string(index).ok_or_else(|| {
            invalid(
                "axml adopt",
                format!("source string index {index} is invalid"),
            )
        })
    }

    fn adopt_namespace(
        &mut self,
        source: &AxmlDocument,
        namespace: Option<u32>,
        declarations: &[u32],
    ) -> Result<Option<u32>> {
        let Some(index) = namespace else {
            return Ok(None);
        };
        let uri = Self::source_string(source, index)?;
        self.namespace_index(&uri).or_else(|| declarations.iter().copied().find(|&index| self.string(index).as_deref() == Some(uri.as_ref()))).map(Some).ok_or_else(|| {
            invalid(
                "axml adopt",
                format!(
                    "the document declares no namespace {uri}; declare it with declareNamespace(prefix, \"{uri}\")"
                ),
            )
        })
    }

    fn adopt_attribute(
        &mut self,
        source: &AxmlDocument,
        attr: &AxmlAttribute,
        resources: Option<&ResourceTable>,
        declarations: &[u32],
    ) -> Result<AxmlAttribute> {
        let uri = attr
            .namespace
            .map(|index| Self::source_string(source, index).map(std::borrow::Cow::into_owned))
            .transpose()?;
        let local = Self::source_string(source, attr.name)?;
        let namespace = self.adopt_namespace(source, attr.namespace, declarations)?;
        let name = match crate::axml::compiler::attribute_resource_id(
            uri.as_deref(),
            &local,
            resources,
        )? {
            Some(id) => self.intern_attribute_name(&local, id),
            None => self.intern_string(&local),
        };
        let value = match attr.value.string_index() {
            Some(index) => ResValue::string(self.adopt_string(source, index)?),
            None => attr.value,
        };
        let mut adopted = AxmlAttribute::new(namespace, name, value);
        adopted.encoded.clone_from(&attr.encoded);
        adopted.roles = attr.roles;
        adopted.raw_value = attr
            .raw_value
            .map(|index| self.adopt_string(source, index))
            .transpose()?;
        Ok(adopted)
    }
}
