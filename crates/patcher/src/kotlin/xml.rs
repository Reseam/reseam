// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! XML documents a patch holds open by handle. Elements are addressed by
//! the index of their start event; elements created but not yet attached
//! live in a pending table and use handles at or above `PENDING_OFFSET`.
//! Closing a document writes it back to the APK.

use std::cell::RefCell;

use boltffi::export;
use reseam_apk::axml::{self, AttributeValue, AxmlAttribute, AxmlDocument, AxmlEvent, ANDROID_NS};
use reseam_apk::{Compression, ResValue, StringPool};

use super::files::with_component;
use super::handles::with_ctx;

const PENDING_OFFSET: u32 = 0x8000_0000;

#[derive(Clone, PartialEq, Eq)]
pub(super) enum DocSource {
    File { component: usize, path: String },
    Manifest { component: usize },
}

struct OpenDoc {
    doc: AxmlDocument,
    source: DocSource,
}

struct PendingElement {
    doc: u32,
    events: Vec<AxmlEvent>,
}

thread_local! {
    static DOCS: RefCell<Vec<Option<OpenDoc>>> = const { RefCell::new(Vec::new()) };
    static PENDING: RefCell<Vec<PendingElement>> = const { RefCell::new(Vec::new()) };
}

pub(super) fn reset() {
    DOCS.with(|docs| docs.borrow_mut().clear());
    PENDING.with(|pending| pending.borrow_mut().clear());
}

/// The handle of the open document for `source`, opening it with `load`
/// when no document is open yet.
pub(super) fn open_source(
    source: &DocSource,
    load: impl FnOnce() -> Option<AxmlDocument>,
) -> Option<u32> {
    let existing = DOCS.with(|docs| {
        docs.borrow()
            .iter()
            .position(|slot| slot.as_ref().is_some_and(|open| open.source == *source))
    });
    if let Some(handle) = existing {
        return Some(handle as u32);
    }
    let doc = load()?;
    DOCS.with(|docs| {
        let mut docs = docs.borrow_mut();
        docs.push(Some(OpenDoc {
            doc,
            source: source.clone(),
        }));
        Some(docs.len() as u32 - 1)
    })
}

pub(super) fn is_open(source: &DocSource) -> bool {
    DOCS.with(|docs| {
        docs.borrow()
            .iter()
            .flatten()
            .any(|open| open.source == *source)
    })
}

pub(super) fn with_source_doc<R>(
    source: &DocSource,
    f: impl FnOnce(&AxmlDocument) -> R,
) -> Option<R> {
    DOCS.with(|docs| {
        let docs = docs.borrow();
        let open = docs.iter().flatten().find(|open| open.source == *source)?;
        Some(f(&open.doc))
    })
}

pub(super) fn with_source_doc_mut<R>(
    source: &DocSource,
    f: impl FnOnce(&mut AxmlDocument) -> R,
) -> Option<R> {
    DOCS.with(|docs| {
        let mut docs = docs.borrow_mut();
        let open = docs
            .iter_mut()
            .flatten()
            .find(|open| open.source == *source)?;
        Some(f(&mut open.doc))
    })
}

fn with_doc<R>(handle: u32, f: impl FnOnce(&AxmlDocument) -> R) -> Option<R> {
    DOCS.with(|docs| Some(f(&docs.borrow().get(handle as usize)?.as_ref()?.doc)))
}

fn with_doc_mut<R>(handle: u32, f: impl FnOnce(&mut AxmlDocument) -> R) -> Option<R> {
    DOCS.with(|docs| {
        Some(f(&mut docs
            .borrow_mut()
            .get_mut(handle as usize)?
            .as_mut()?
            .doc))
    })
}

fn pending_index(handle: u32) -> Option<usize> {
    handle.checked_sub(PENDING_OFFSET).map(|i| i as usize)
}

fn push_pending(doc: u32, events: Vec<AxmlEvent>) -> u32 {
    PENDING.with(|pending| {
        let mut pending = pending.borrow_mut();
        pending.push(PendingElement { doc, events });
        PENDING_OFFSET + pending.len() as u32 - 1
    })
}

/// Removes a pending element from the table, leaving its handle dangling.
fn take_pending(handle: u32) -> Option<PendingElement> {
    let index = pending_index(handle)?;
    PENDING.with(|pending| {
        let mut pending = pending.borrow_mut();
        let slot = pending.get_mut(index)?;
        let events = std::mem::take(&mut slot.events);
        (!events.is_empty()).then_some(PendingElement {
            doc: slot.doc,
            events,
        })
    })
}

/// Read access to the start element `el` names, in the document or pending.
fn with_element<R>(
    doc: u32,
    el: u32,
    f: impl FnOnce(&AxmlDocument, &[AxmlAttribute]) -> R,
) -> Option<R> {
    match pending_index(el) {
        Some(index) => PENDING.with(|pending| {
            let pending = pending.borrow();
            let element = pending.get(index)?;
            let AxmlEvent::StartElement { attributes, .. } = element.events.first()? else {
                return None;
            };
            with_doc(element.doc, |doc| f(doc, attributes))
        }),
        None => with_doc(doc, |doc| Some(f(doc, doc.attributes(el as usize)))).flatten(),
    }
}

/// Mutable access to the attributes of the start element `el` names, with
/// the document's string pool and its `android:` namespace index.
fn with_attributes_mut<R>(
    doc: u32,
    el: u32,
    f: impl FnOnce(&mut StringPool, Option<u32>, &mut Vec<AxmlAttribute>) -> R,
) -> Option<R> {
    match pending_index(el) {
        Some(index) => PENDING.with(|pending| {
            let mut pending = pending.borrow_mut();
            let element = pending.get_mut(index)?;
            let AxmlEvent::StartElement { attributes, .. } = element.events.first_mut()? else {
                return None;
            };
            with_doc_mut(element.doc, |doc| {
                let ns = android_ns(doc);
                f(&mut doc.string_pool, ns, attributes)
            })
        }),
        None => with_doc_mut(doc, |document| {
            let ns = android_ns(document);
            let AxmlDocument {
                string_pool,
                elements,
                ..
            } = document;
            let AxmlEvent::StartElement { attributes, .. } = elements.get_mut(el as usize)? else {
                return None;
            };
            Some(f(string_pool, ns, attributes))
        })
        .flatten(),
    }
}

fn android_ns(doc: &AxmlDocument) -> Option<u32> {
    doc.elements.iter().find_map(|event| match event {
        AxmlEvent::StartNamespace { uri, .. }
            if doc.string(*uri).as_deref() == Some(ANDROID_NS) =>
        {
            Some(*uri)
        }
        _ => None,
    })
}

/// `android:name` -> (android namespace, `name`); anything else is unqualified.
fn split_name(ns: Option<u32>, name: &str) -> (Option<u32>, &str) {
    match name.strip_prefix("android:") {
        Some(local) => (ns, local),
        None => (None, name),
    }
}

fn set_or_add(
    attributes: &mut Vec<AxmlAttribute>,
    namespace: Option<u32>,
    name: u32,
    value: ResValue,
) {
    match attributes
        .iter_mut()
        .find(|attr| attr.name == name && attr.namespace == namespace)
    {
        Some(attr) => attr.set_value(value),
        None => attributes.push(AxmlAttribute::new(namespace, name, value)),
    }
}

fn set_attribute_value(
    doc: u32,
    el: u32,
    name: &str,
    value: impl FnOnce(&mut StringPool) -> ResValue,
) {
    with_attributes_mut(doc, el, |pool, ns, attributes| {
        let (namespace, local) = split_name(ns, name);
        let name = pool.intern(local);
        set_or_add(attributes, namespace, name, value(pool));
    });
}

fn attribute_text(doc: &AxmlDocument, attributes: &[AxmlAttribute], name: &str) -> Option<String> {
    let (namespace, local) = split_name(android_ns(doc), name);
    let attr = attributes.iter().find(|attr| {
        attr.namespace == namespace && doc.string(attr.name).as_deref() == Some(local)
    })?;
    Some(match attr.value.kind {
        ResValue::STRING => doc.attribute_string(attr)?.into_owned(),
        ResValue::INT_DEC => (attr.value.data as i32).to_string(),
        ResValue::INT_BOOLEAN => (attr.value.data != 0).to_string(),
        ResValue::REFERENCE => format!("@0x{:08x}", attr.value.data),
        ResValue::INT_HEX => format!("0x{:08x}", attr.value.data),
        _ => attr.value.data.to_string(),
    })
}

fn subtree(doc: &AxmlDocument, start: usize) -> Vec<AxmlEvent> {
    let end = doc.find_end_element(start).unwrap_or(start);
    doc.elements[start..=end].to_vec()
}

/// Detaches the element at `start` and returns its events.
fn detach(doc: &mut AxmlDocument, start: usize) -> Vec<AxmlEvent> {
    let end = doc.find_end_element(start).unwrap_or(start);
    doc.elements.drain(start..=end).collect()
}

/// Opens `apk_path` from the component (base when `None`) as a document.
#[export]
pub fn xml_open(component: Option<String>, apk_path: String) -> Option<u32> {
    with_component(component, |ctx, index| {
        let source = DocSource::File {
            component: index,
            path: apk_path.clone(),
        };
        open_source(&source, || {
            let data = ctx.read_file(index, &apk_path).ok().flatten()?;
            AxmlDocument::parse(&data).ok()
        })
    })
    .flatten()
}

/// Writes the document back to the APK and releases its handle.
#[export]
pub fn xml_close(doc: u32) {
    let Some(open) = DOCS.with(|docs| {
        docs.borrow_mut()
            .get_mut(doc as usize)
            .and_then(Option::take)
    }) else {
        return;
    };
    with_ctx(|ctx| {
        let outcome = match open.source {
            DocSource::File { component, path } => open
                .doc
                .serialize()
                .map_err(|e| e.to_string())
                .and_then(|data| {
                    ctx.inject_file(component, &path, data, Compression::Deflated)
                        .map_err(|e| e.to_string())
                }),
            DocSource::Manifest { component } => ctx
                .component_mut(component)
                .map_err(|e| e.to_string())
                .map(|c| {
                    *c.manifest_mut() = open.doc;
                }),
        };
        if let Err(error) = outcome {
            ctx.log().warn(format!("xml close: {error}"));
        }
    });
}

#[export]
pub fn xml_root(doc: u32) -> u32 {
    with_doc(doc, |doc| doc.root().unwrap_or(0) as u32).unwrap_or(0)
}

#[export]
pub fn xml_find_by_tag(doc: u32, tag: String) -> Vec<u32> {
    with_doc(doc, |doc| {
        (0..doc.elements.len())
            .filter(|&i| doc.element_name(i).as_deref() == Some(tag.as_str()))
            .map(|i| i as u32)
            .collect()
    })
    .unwrap_or_default()
}

#[export]
pub fn xml_find_by_attribute(doc: u32, attr_name: String, attr_value: String) -> Vec<u32> {
    with_doc(doc, |doc| {
        doc.elements
            .iter()
            .enumerate()
            .filter(|(_, event)| match event {
                AxmlEvent::StartElement { attributes, .. } => {
                    attribute_text(doc, attributes, &attr_name).as_deref()
                        == Some(attr_value.as_str())
                }
                _ => false,
            })
            .map(|(i, _)| i as u32)
            .collect()
    })
    .unwrap_or_default()
}

#[export]
pub fn xml_children(doc: u32, el: u32) -> Vec<u32> {
    with_doc(doc, |doc| {
        let start = el as usize;
        let Some(end) = doc.find_end_element(start) else {
            return Vec::new();
        };
        let mut depth = 0usize;
        let mut children = Vec::new();
        for (i, event) in doc.elements.iter().enumerate().take(end).skip(start + 1) {
            match event {
                AxmlEvent::StartElement { .. } => {
                    if depth == 0 {
                        children.push(i as u32);
                    }
                    depth += 1;
                }
                AxmlEvent::EndElement { .. } => depth -= 1,
                _ => {}
            }
        }
        children
    })
    .unwrap_or_default()
}

#[export]
pub fn xml_parent(doc: u32, el: u32) -> Option<u32> {
    with_doc(doc, |doc| {
        let mut depth = 0i32;
        for i in (0..el as usize).rev() {
            match doc.elements.get(i)? {
                AxmlEvent::EndElement { .. } => depth += 1,
                AxmlEvent::StartElement { .. } if depth == 0 => return Some(i as u32),
                AxmlEvent::StartElement { .. } => depth -= 1,
                _ => {}
            }
        }
        None
    })
    .flatten()
}

#[export]
pub fn xml_tag_name(doc: u32, el: u32) -> String {
    match pending_index(el) {
        Some(index) => PENDING.with(|pending| {
            let pending = pending.borrow();
            let element = pending.get(index)?;
            let AxmlEvent::StartElement { name, .. } = element.events.first()? else {
                return None;
            };
            with_doc(element.doc, |doc| doc.string(*name).map(|s| s.into_owned())).flatten()
        }),
        None => with_doc(doc, |doc| {
            doc.element_name(el as usize).map(|s| s.into_owned())
        })
        .flatten(),
    }
    .unwrap_or_default()
}

#[export]
pub fn xml_get_attribute(doc: u32, el: u32, name: String) -> Option<String> {
    with_element(doc, el, |doc, attributes| {
        attribute_text(doc, attributes, &name)
    })
    .flatten()
}

/// Sets an attribute from text, parsing literals and resource references the
/// way the XML compiler does.
#[export]
pub fn xml_set_attribute(doc: u32, el: u32, name: String, value: String) {
    let parsed = with_ctx(|ctx| {
        let resources = ctx.apk_mut().base_mut().resources_mut().ok().flatten();
        axml::parse_attribute_value(&value, resources).ok()
    });
    let Some(parsed) = parsed else {
        return;
    };
    set_attribute_value(doc, el, &name, |pool| match parsed {
        AttributeValue::Value(value) => value,
        AttributeValue::Text => ResValue::string(pool.intern(&value)),
    });
}

#[export]
pub fn xml_set_attribute_ref(doc: u32, el: u32, name: String, res_id: u32) {
    set_attribute_value(doc, el, &name, |_| ResValue::reference(res_id));
}

#[export]
pub fn xml_remove_attribute(doc: u32, el: u32, name: String) {
    with_attributes_mut(doc, el, |pool, ns, attributes| {
        let (namespace, local) = split_name(ns, &name);
        if let Some(name) = pool.find(local) {
            attributes.retain(|attr| !(attr.name == name && attr.namespace == namespace));
        }
    });
}

/// A detached element; attach it with `xml_append_child` or `xml_insert_before`.
#[export]
pub fn xml_create_element(doc: u32, tag: String) -> u32 {
    with_doc_mut(doc, |document| {
        let name = document.intern_string(&tag);
        push_pending(
            doc,
            vec![
                AxmlEvent::StartElement {
                    namespace: None,
                    name,
                    attributes: Vec::new(),
                },
                AxmlEvent::EndElement {
                    namespace: None,
                    name,
                },
            ],
        )
    })
    .unwrap_or(0)
}

#[export]
pub fn xml_append_child(doc: u32, parent: u32, child: u32) {
    if let Some(pending) = take_pending(child) {
        if let Some(parent_index) = pending_index(parent) {
            PENDING.with(|table| {
                let mut table = table.borrow_mut();
                let Some(parent) = table.get_mut(parent_index) else {
                    return;
                };
                let end = parent.events.len().saturating_sub(1);
                parent.events.splice(end..end, pending.events);
            });
            return;
        }
        with_doc_mut(pending.doc, |document| {
            if let Some(end) = document.find_end_element(parent as usize) {
                document.elements.splice(end..end, pending.events);
            }
        });
        return;
    }
    with_doc_mut(doc, |document| {
        let child = child as usize;
        if child >= document.elements.len() {
            return;
        }
        let events = detach(document, child);
        let parent = parent as usize
            - if parent as usize > child {
                events.len()
            } else {
                0
            };
        if let Some(end) = document.find_end_element(parent) {
            document.elements.splice(end..end, events);
        }
    });
}

#[export]
pub fn xml_insert_before(doc: u32, child: u32, before: u32) {
    if let Some(pending) = take_pending(child) {
        with_doc_mut(pending.doc, |document| {
            let before = before as usize;
            if before <= document.elements.len() {
                document.elements.splice(before..before, pending.events);
            }
        });
        return;
    }
    with_doc_mut(doc, |document| {
        let child = child as usize;
        if child >= document.elements.len() {
            return;
        }
        let events = detach(document, child);
        let before = before as usize
            - if before as usize > child {
                events.len()
            } else {
                0
            };
        if before <= document.elements.len() {
            document.elements.splice(before..before, events);
        }
    });
}

#[export]
pub fn xml_remove_element(doc: u32, el: u32) {
    with_doc_mut(doc, |document| document.remove_element(el as usize));
}

/// A detached copy of an element, with or without its children.
#[export]
pub fn xml_clone_element(doc: u32, el: u32, deep: bool) -> u32 {
    with_doc(doc, |document| {
        let start = el as usize;
        let AxmlEvent::StartElement {
            namespace,
            name,
            attributes,
        } = document.elements.get(start)?
        else {
            return None;
        };
        let events = if deep {
            subtree(document, start)
        } else {
            vec![
                AxmlEvent::StartElement {
                    namespace: *namespace,
                    name: *name,
                    attributes: attributes.clone(),
                },
                AxmlEvent::EndElement {
                    namespace: *namespace,
                    name: *name,
                },
            ]
        };
        Some(push_pending(doc, events))
    })
    .flatten()
    .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_test_doc(f: impl FnOnce(u32)) -> AxmlDocument {
        let strings = [ANDROID_NS, "android", "LinearLayout"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let doc = AxmlDocument {
            string_pool: StringPool::new(strings, true),
            resource_ids: Vec::new(),
            elements: vec![
                AxmlEvent::StartNamespace {
                    prefix: Some(1),
                    uri: 0,
                },
                AxmlEvent::StartElement {
                    namespace: None,
                    name: 2,
                    attributes: Vec::new(),
                },
                AxmlEvent::EndElement {
                    namespace: None,
                    name: 2,
                },
                AxmlEvent::EndNamespace {
                    prefix: Some(1),
                    uri: 0,
                },
            ],
        };
        reset();
        let source = DocSource::File {
            component: 0,
            path: "res/layout/test.xml".into(),
        };
        let handle = open_source(&source, || Some(doc)).unwrap();
        f(handle);
        let document = with_doc(handle, Clone::clone).unwrap();
        reset();
        document
    }

    #[test]
    fn typed_attributes_and_nested_pending_elements() {
        let document = with_test_doc(|doc| {
            set_attribute_value(doc, 1, "android:padding", |_| ResValue::int(16));
            set_attribute_value(doc, 1, "android:enabled", |_| ResValue::boolean(true));
            let parent = xml_create_element(doc, "activity".into());
            let filter = xml_create_element(doc, "intent-filter".into());
            let action = xml_create_element(doc, "action".into());
            xml_set_attribute_ref(doc, action, "android:name".into(), 0x7f00_0001);
            xml_append_child(doc, filter, action);
            xml_append_child(doc, parent, filter);
            xml_append_child(doc, 1, parent);
            assert_eq!(xml_children(doc, 1).len(), 1);
        });
        let names: Vec<_> = (0..document.elements.len())
            .filter_map(|i| document.element_name(i).map(|s| s.into_owned()))
            .collect();
        assert_eq!(
            names,
            ["LinearLayout", "activity", "intent-filter", "action"]
        );
        let root_attrs = document.attributes(1);
        assert_eq!(root_attrs.len(), 2);
        assert_eq!(root_attrs[0].value, ResValue::int(16));
        assert_eq!(root_attrs[0].namespace, Some(0));
        assert_eq!(root_attrs[1].value, ResValue::boolean(true));
        assert_eq!(
            document.attributes(4)[0].value,
            ResValue::reference(0x7f00_0001)
        );
    }
}
