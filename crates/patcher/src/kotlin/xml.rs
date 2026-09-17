// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! XML documents a patch holds open by handle. Elements are addressed by
//! the index of their start event; elements created but not yet attached
//! live in a pending table and use handles at or above `PENDING_OFFSET`.
//! Closing a document writes it back to the APK.

use std::cell::{Cell, RefCell};

use boltffi::export;
use reseam_apk::axml::{self, AttributeValue, AxmlAttribute, AxmlDocument, AxmlEvent};
use reseam_apk::{Compression, ResValue, ResourceTable, StringPool};

use super::files::with_component;
use super::handles::with_ctx;

const PENDING_OFFSET: u32 = 0x8000_0000;

#[derive(Clone, PartialEq, Eq)]
pub(super) enum DocSource {
    File {
        component: usize,
        path: String,
    },
    Manifest {
        component: usize,
    },
    /// A document compiled from text, written back nowhere.
    Memory {
        id: u32,
    },
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
    static COMPILED: Cell<u32> = const { Cell::new(0) };
}

pub(super) fn reset() {
    DOCS.with(|docs| docs.borrow_mut().clear());
    PENDING.with(|pending| pending.borrow_mut().clear());
    COMPILED.with(|count| count.set(0));
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

/// Runs `f` with `target` open for writing and `source` readable beside it,
/// which one `RefCell` over every document cannot hand out at once. The source
/// is lifted out of the table for the call and put back after.
fn with_two_docs<R>(
    target: u32,
    source: u32,
    f: impl FnOnce(&mut AxmlDocument, &AxmlDocument) -> R,
) -> Option<R> {
    if target == source {
        return None;
    }
    let taken = DOCS.with(|docs| docs.borrow_mut().get_mut(source as usize)?.take())?;
    let result = with_doc_mut(target, |doc| f(doc, &taken.doc));
    DOCS.with(|docs| {
        if let Some(slot) = docs.borrow_mut().get_mut(source as usize) {
            *slot = Some(taken);
        }
    });
    result
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

/// Mutable access to the attributes of the start element `el` names, together
/// with the document they belong to. The attributes are lifted out for the
/// call so the document itself stays reachable: binding an attribute name
/// writes to the string pool and the resource id map.
fn with_attributes_mut<R>(
    doc: u32,
    el: u32,
    f: impl FnOnce(&mut AxmlDocument, &mut Vec<AxmlAttribute>) -> R,
) -> Option<R> {
    match pending_index(el) {
        Some(index) => {
            let (owner, mut attributes) = PENDING.with(|pending| {
                let mut pending = pending.borrow_mut();
                let element = pending.get_mut(index)?;
                let AxmlEvent::StartElement { attributes, .. } = element.events.first_mut()? else {
                    return None;
                };
                Some((element.doc, std::mem::take(attributes)))
            })?;
            let result = with_doc_mut(owner, |document| f(document, &mut attributes));
            PENDING.with(|pending| {
                if let Some(AxmlEvent::StartElement {
                    attributes: slot, ..
                }) = pending
                    .borrow_mut()
                    .get_mut(index)
                    .and_then(|element| element.events.first_mut())
                {
                    *slot = attributes;
                }
            });
            result
        }
        None => {
            let mut attributes = with_doc_mut(doc, |document| {
                match document.elements.get_mut(el as usize)? {
                    AxmlEvent::StartElement { attributes, .. } => Some(std::mem::take(attributes)),
                    _ => None,
                }
            })
            .flatten()?;
            let result = with_doc_mut(doc, |document| f(document, &mut attributes));
            with_doc_mut(doc, |document| {
                if let Some(AxmlEvent::StartElement {
                    attributes: slot, ..
                }) = document.elements.get_mut(el as usize)
                {
                    *slot = attributes;
                }
            });
            result
        }
    }
}

/// Replaces the value of the matching attribute, or adds one in resource id
/// order. aapt writes attributes in that order and the framework's lookup
/// walks them expecting it.
fn set_or_add(
    doc: &AxmlDocument,
    attributes: &mut Vec<AxmlAttribute>,
    namespace: Option<u32>,
    name: u32,
    value: ResValue,
) {
    if let Some(attr) = attributes
        .iter_mut()
        .find(|attr| attr.name == name && attr.namespace == namespace)
    {
        attr.set_value(value);
        return;
    }
    doc.insert_attribute(attributes, AxmlAttribute::new(namespace, name, value));
}

/// `resources` resolves the ids of attributes the app declares itself, which
/// is every namespace other than `android:`.
fn set_attribute_value(
    doc: u32,
    el: u32,
    name: &str,
    resources: Option<&mut ResourceTable>,
    value: impl FnOnce(
        &mut StringPool,
        Option<u32>,
        Option<&mut ResourceTable>,
    ) -> Result<ResValue, String>,
) -> Result<(), String> {
    with_attributes_mut(doc, el, |document, attributes| {
        let (namespace, name) = document
            .bind_attribute_name(name, resources.as_deref())
            .map_err(|error| error.to_string())?;
        let attr = namespace.and_then(|_| document.resource_id_for(name));
        let value = value(&mut document.string_pool, attr, resources)?;
        set_or_add(document, attributes, namespace, name, value);
        Ok(())
    })
    .unwrap_or_else(|| Err(format!("element {el} is not an element of document {doc}")))
}

/// `prefix:local` -> (the namespace the document declares for the prefix,
/// `local`); an unprefixed name is unqualified. An undeclared prefix matches
/// no attribute, which is what reading and removing it should report.
fn split_name<'a>(doc: &AxmlDocument, name: &'a str) -> (Option<u32>, &'a str) {
    match name.split_once(':') {
        Some((prefix, local)) => (doc.declared_namespace(prefix), local),
        None => (None, name),
    }
}

fn attribute_text(doc: &AxmlDocument, attributes: &[AxmlAttribute], name: &str) -> Option<String> {
    let (namespace, local) = split_name(doc, name);
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
            DocSource::Memory { .. } => Ok(()),
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
/// way the XML compiler does. Fails when the attribute cannot be bound to a
/// resource id, since the inflater would ignore it.
#[export]
pub fn xml_set_attribute(doc: u32, el: u32, name: String, value: String) -> Result<(), String> {
    with_ctx(|ctx| {
        let resources = ctx.apk_mut().base_mut().resources_mut().ok().flatten();
        set_attribute_value(doc, el, &name, resources, |pool, attr, resources| {
            text_attribute(pool, attr, resources, &value)
                .map_err(|error| format!("attribute {name}: {error}"))
        })
    })
}

fn text_attribute(
    pool: &mut StringPool,
    attr: Option<u32>,
    resources: Option<&mut ResourceTable>,
    text: &str,
) -> reseam_apk::Result<ResValue> {
    Ok(match axml::parse_attribute_value(text, attr, resources)? {
        AttributeValue::Value(value) => value,
        AttributeValue::Text => ResValue::string(pool.intern(text)),
    })
}

#[export]
pub fn xml_set_attribute_ref(doc: u32, el: u32, name: String, res_id: u32) -> Result<(), String> {
    with_ctx(|ctx| {
        let resources = ctx.apk_mut().base_mut().resources_mut().ok().flatten();
        set_attribute_value(doc, el, &name, resources, |_, _, _| {
            Ok(ResValue::reference(res_id))
        })
    })
}

#[export]
pub fn xml_remove_attribute(doc: u32, el: u32, name: String) {
    with_attributes_mut(doc, el, |document, attributes| {
        let (namespace, local) = split_name(document, &name);
        if let Some(name) = document.string_pool.find(local) {
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

/// Moves `child` to the end of `parent`'s children. `child` is a detached
/// element (which is then used up) or one of the document; `parent` is either.
#[export]
pub fn xml_append_child(doc: u32, parent: u32, child: u32) -> Result<(), String> {
    if pending_index(child).is_some() {
        let pending = take_pending(child).ok_or_else(|| used_up(child))?;
        if let Some(parent_index) = pending_index(parent) {
            return PENDING.with(|table| {
                let mut table = table.borrow_mut();
                let parent = table
                    .get_mut(parent_index)
                    .filter(|parent| !parent.events.is_empty())
                    .ok_or_else(|| used_up(parent))?;
                let end = parent.events.len() - 1;
                parent.events.splice(end..end, pending.events);
                Ok(())
            });
        }
        return with_doc_mut(pending.doc, |document| {
            let end = end_of(document, parent, pending.doc)?;
            document.elements.splice(end..end, pending.events);
            Ok(())
        })
        .unwrap_or_else(|| Err(format!("document {} is closed", pending.doc)));
    }
    with_doc_mut(doc, |document| {
        end_of(document, child, doc)?;
        end_of(document, parent, doc)?;
        let events = detach(document, child as usize);
        let parent = parent as usize - if parent > child { events.len() } else { 0 };
        let end = end_of(document, parent as u32, doc)?;
        document.elements.splice(end..end, events);
        Ok(())
    })
    .unwrap_or_else(|| Err(format!("document {doc} is closed")))
}

/// Moves `child` in front of `before`, an element of the document, and returns
/// the handle `child` has there. Handles resolved earlier may have moved.
#[export]
pub fn xml_insert_before(doc: u32, child: u32, before: u32) -> Result<u32, String> {
    if pending_index(before).is_some() {
        return Err(format!(
            "element {before} is not in the document; insert it before inserting next to it"
        ));
    }
    let pending = match pending_index(child) {
        Some(_) => Some(take_pending(child).ok_or_else(|| used_up(child))?),
        None => None,
    };
    let owner = pending.as_ref().map_or(doc, |pending| pending.doc);
    with_doc_mut(owner, |document| {
        end_of(document, before, owner)?;
        let (events, before) = match pending {
            Some(pending) => (pending.events, before as usize),
            None => {
                end_of(document, child, owner)?;
                let events = detach(document, child as usize);
                let shift = if before > child { events.len() } else { 0 };
                (events, before as usize - shift)
            }
        };
        document.elements.splice(before..before, events);
        Ok(before as u32)
    })
    .unwrap_or_else(|| Err(format!("document {owner} is closed")))
}

/// Where the start element `el` of `document` ends, or why `el` names none.
fn end_of(document: &AxmlDocument, el: u32, doc: u32) -> Result<usize, String> {
    document
        .find_end_element(el as usize)
        .ok_or_else(|| format!("element {el} is not an element of document {doc}"))
}

fn used_up(el: u32) -> String {
    format!("element {el} was already attached; use the handle the attaching call returned")
}

#[export]
pub fn xml_remove_element(doc: u32, el: u32) {
    with_doc_mut(doc, |document| document.remove_element(el as usize));
}

/// Compiles XML text into a document of its own. It is backed by no APK entry,
/// so closing it discards it; it is the source a patch grafts a subtree out of.
#[export]
pub fn xml_compile(text: String) -> Result<u32, String> {
    with_ctx(|ctx| {
        let resources = ctx.apk_mut().base_mut().resources_mut().ok().flatten();
        let doc = axml::build_document(&text, resources).map_err(|error| error.to_string())?;
        let id = COMPILED.with(|count| {
            let id = count.get();
            count.set(id + 1);
            id
        });
        open_source(&DocSource::Memory { id }, || Some(doc))
            .ok_or_else(|| "could not open the compiled document".to_string())
    })
}

/// Declares `prefix` for `uri` on a document that lacks it, which an adopted
/// attribute in that namespace needs. Namespaces wrap the event stream, so this
/// moves the index of every element resolved before the call.
#[export]
pub fn xml_declare_namespace(doc: u32, prefix: String, uri: String) -> Result<(), String> {
    with_doc_mut(doc, |document| {
        document
            .declare_namespace(&prefix, &uri)
            .map_err(|error| error.to_string())
    })
    .unwrap_or_else(|| Err(format!("no document {doc} is open")))
}

/// A detached deep copy of an element of another document, in this document's
/// strings and namespaces, with every attribute rebound to the id this document
/// resolves it by.
#[export]
pub fn xml_adopt(doc: u32, source_doc: u32, source_el: u32) -> Result<u32, String> {
    let events = with_ctx(|ctx| {
        let resources = ctx.apk_mut().base_mut().resources_mut().ok().flatten();
        with_two_docs(doc, source_doc, |target, source| {
            let events = match pending_index(source_el) {
                Some(index) => PENDING
                    .with(|pending| pending.borrow().get(index).map(|el| el.events.clone()))
                    .ok_or_else(|| format!("no element {source_el} is pending"))?,
                None => {
                    let start = source_el as usize;
                    if start >= source.elements.len() {
                        return Err(format!(
                            "element {source_el} is not an element of document {source_doc}"
                        ));
                    }
                    subtree(source, start)
                }
            };
            target
                .adopt(source, &events, resources.map(|table| &*table))
                .map_err(|error| error.to_string())
        })
        .unwrap_or_else(|| {
            Err(format!(
                "cannot adopt into document {doc} from {source_doc}: use clone within one document"
            ))
        })
    })?;
    Ok(push_pending(doc, events))
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
    use reseam_apk::axml::{android_attr_res_id, android_attrs, ANDROID_NS};

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
    fn inserting_before_a_just_inserted_element_uses_the_returned_handle() {
        let document = with_test_doc(|doc| {
            let first = xml_create_element(doc, "first".into());
            let second = xml_create_element(doc, "second".into());
            let anchor = xml_create_element(doc, "anchor".into());
            xml_append_child(doc, 1, anchor).unwrap();
            let anchor = xml_children(doc, 1)[0];
            let first = xml_insert_before(doc, first, anchor).unwrap();
            xml_insert_before(doc, second, first).unwrap();

            assert!(xml_insert_before(doc, second, first)
                .unwrap_err()
                .contains("already attached"));
            assert!(xml_append_child(doc, 99, 1)
                .unwrap_err()
                .contains("not an element of document"));
        });
        let names: Vec<_> = (0..document.elements.len())
            .filter_map(|i| document.element_name(i).map(|s| s.into_owned()))
            .collect();
        assert!(
            names.ends_with(&[
                "second".to_string(),
                "first".to_string(),
                "anchor".to_string()
            ]),
            "{names:?}"
        );
    }

    #[test]
    fn text_attributes_take_the_enum_and_flag_names_of_their_attribute() {
        let document = with_test_doc(|doc| {
            for (name, text) in [
                ("android:scaleType", "center"),
                ("android:gravity", "top|start"),
                ("android:text", "center"),
            ] {
                set_attribute_value(doc, 1, name, None, |pool, attr, resources| {
                    text_attribute(pool, attr, resources, text).map_err(|error| error.to_string())
                })
                .unwrap();
            }
        });
        let value = |local: &str| {
            let id = android_attr_res_id(local).unwrap();
            document
                .attributes(1)
                .iter()
                .find(|attr| document.resource_id_for(attr.name) == Some(id))
                .unwrap()
                .value
        };
        assert_eq!(value("scaleType"), ResValue::int(5));
        assert_eq!(value("gravity"), ResValue::hex(0x30 | 0x0080_0003));
        assert_eq!(value("text").kind, ResValue::STRING);
    }

    #[test]
    fn typed_attributes_and_nested_pending_elements() {
        let document = with_test_doc(|doc| {
            set_attribute_value(doc, 1, "android:padding", None, |_, _, _| {
                Ok(ResValue::int(16))
            })
            .unwrap();
            set_attribute_value(doc, 1, "android:enabled", None, |_, _, _| {
                Ok(ResValue::boolean(true))
            })
            .unwrap();
            let parent = xml_create_element(doc, "activity".into());
            let filter = xml_create_element(doc, "intent-filter".into());
            let action = xml_create_element(doc, "action".into());
            set_attribute_value(doc, action, "android:name", None, |_, _, _| {
                Ok(ResValue::reference(0x7f00_0001))
            })
            .unwrap();
            xml_append_child(doc, filter, action).unwrap();
            xml_append_child(doc, parent, filter).unwrap();
            xml_append_child(doc, 1, parent).unwrap();
            assert_eq!(xml_children(doc, 1).len(), 1);
        });
        let names: Vec<_> = (0..document.elements.len())
            .filter_map(|i| document.element_name(i).map(|s| s.into_owned()))
            .collect();
        assert_eq!(
            names,
            ["LinearLayout", "activity", "intent-filter", "action"]
        );
        // Written padding first, but attributes are kept in resource id order,
        // and every one of them carries the framework id the inflater reads.
        let root_attrs = document.attributes(1);
        assert_eq!(root_attrs.len(), 2);
        assert_eq!(root_attrs[0].value, ResValue::boolean(true));
        assert_eq!(root_attrs[0].namespace, Some(0));
        assert_eq!(
            document.resource_id_for(root_attrs[0].name),
            Some(android_attrs::ATTR_ENABLED)
        );
        assert_eq!(root_attrs[1].value, ResValue::int(16));
        assert_eq!(
            document.resource_id_for(root_attrs[1].name),
            android_attr_res_id("padding")
        );
        let action_attr = &document.attributes(4)[0];
        assert_eq!(action_attr.value, ResValue::reference(0x7f00_0001));
        assert_eq!(
            document.resource_id_for(action_attr.name),
            Some(android_attrs::ATTR_NAME)
        );
    }
}
