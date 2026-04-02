use stitch_apk::axml::{AxmlAttribute, AxmlDocument, AxmlEvent, TypedValue};

use boltffi::export;

use super::{with_ctx, PENDING_ELEMENTS, XML_DOCUMENTS};

const PENDING_OFFSET: u32 = 0x8000_0000;

pub(crate) struct PendingElement {
    doc_idx: u32,
    events: Vec<AxmlEvent>,
}

fn element_end(doc: &AxmlDocument, start: usize) -> usize {
    doc.find_end_element(start).unwrap_or(start)
}

fn extract_subtree(doc: &AxmlDocument, start: usize) -> Vec<AxmlEvent> {
    let end = element_end(doc, start);
    doc.elements[start..=end].to_vec()
}

const ANDROID_NS_URI: &str = "http://schemas.android.com/apk/res/android";

fn resolve_namespace<'a>(
    document: &AxmlDocument,
    name: &'a str,
) -> (Option<u32>, &'a str) {
    if let Some(local) = name.strip_prefix("android:") {
        let ns_idx = document
            .elements
            .iter()
            .find_map(|e| {
                if let AxmlEvent::StartNamespace { uri, .. } = e {
                    if document.string(*uri).map_or(false, |s| s == ANDROID_NS_URI) {
                        return Some(*uri);
                    }
                }
                None
            });
        (ns_idx, local)
    } else {
        (None, name)
    }
}

fn attr_matches(document: &AxmlDocument, attr: &AxmlAttribute, ns: Option<u32>, local: &str) -> bool {
    let name_matches = document.string(attr.name).map_or(false, |s| s == local);
    let ns_matches = match (attr.namespace, ns) {
        (None, None) => true,
        (Some(a), Some(b)) => a == b,
        _ => false,
    };
    name_matches && ns_matches
}

fn get_attr_value(
    document: &AxmlDocument,
    attributes: &[AxmlAttribute],
    name: &str,
) -> Option<String> {
    let (ns, local) = resolve_namespace(document, name);
    for attr in attributes {
        if attr_matches(document, attr, ns, local) {
            return match &attr.typed_value {
                TypedValue::String(si) => document.string(*si).map(|s| s.to_string()),
                TypedValue::Int(v) => Some(v.to_string()),
                TypedValue::Bool(v) => Some(v.to_string()),
                TypedValue::Reference(v) => Some(format!("@0x{v:08x}")),
                TypedValue::Hex(v) => Some(format!("0x{v:08x}")),
                TypedValue::Other { data, .. } => Some(data.to_string()),
            };
        }
    }
    None
}

fn set_or_add_attr(
    attributes: &mut Vec<AxmlAttribute>,
    ns: Option<u32>,
    name_idx: u32,
    val_idx: u32,
) {
    for attr in attributes.iter_mut() {
        if attr.name == name_idx && attr.namespace == ns {
            attr.raw_value = Some(val_idx);
            attr.typed_value = TypedValue::String(val_idx);
            return;
        }
    }
    attributes.push(AxmlAttribute {
        namespace: ns,
        name: name_idx,
        raw_value: Some(val_idx),
        typed_value: TypedValue::String(val_idx),
    });
}

fn insert_events_at(doc: &mut AxmlDocument, pos: usize, events: Vec<AxmlEvent>) {
    for (j, event) in events.into_iter().enumerate() {
        doc.elements.insert(pos + j, event);
    }
}

#[export]
pub fn xml_open(apk_path: String) -> Option<u32> {
    with_ctx(|ctx| {
        let data = ctx.apk().read_entry(&apk_path).ok()?;
        let doc = AxmlDocument::parse(&data).ok()?;
        XML_DOCUMENTS.with(|docs| {
            let mut docs = docs.borrow_mut();
            let handle = docs.len() as u32;
            docs.push(Some((doc, apk_path)));
            Some(handle)
        })
    })
}

#[export]
pub fn xml_close(doc: u32) {
    XML_DOCUMENTS.with(|docs| {
        let mut docs = docs.borrow_mut();
        let idx = doc as usize;
        if let Some(Some((document, path))) = docs.get(idx) {
            match document.serialize() {
                Ok(data) => {
                    let path = path.clone();
                    with_ctx(|ctx| ctx.inject_file(&path, data));
                }
                Err(e) => {
                    let path = path.clone();
                    with_ctx(|ctx| {
                        ctx.log()
                            .warn(format!("xml close: failed to serialize {path}: {e}"))
                    });
                }
            }
        }
        if let Some(slot) = docs.get_mut(idx) {
            *slot = None;
        }
    });
}

#[export]
pub fn xml_root(doc: u32) -> u32 {
    XML_DOCUMENTS.with(|docs| {
        let docs = docs.borrow();
        let idx = doc as usize;
        if let Some(Some((document, _))) = docs.get(idx) {
            for (i, event) in document.elements.iter().enumerate() {
                if matches!(event, AxmlEvent::StartElement { .. }) {
                    return i as u32;
                }
            }
        }
        0
    })
}

#[export]
pub fn xml_find_by_tag(doc: u32, tag: String) -> Vec<u32> {
    XML_DOCUMENTS.with(|docs| {
        let docs = docs.borrow();
        let idx = doc as usize;
        let mut results = Vec::new();
        if let Some(Some((document, _))) = docs.get(idx) {
            for (i, event) in document.elements.iter().enumerate() {
                if let AxmlEvent::StartElement { name, .. } = event {
                    if document.string(*name).map_or(false, |s| s == tag) {
                        results.push(i as u32);
                    }
                }
            }
        }
        results
    })
}

#[export]
pub fn xml_find_by_attribute(doc: u32, attr_name: String, attr_value: String) -> Vec<u32> {
    XML_DOCUMENTS.with(|docs| {
        let docs = docs.borrow();
        let idx = doc as usize;
        let mut results = Vec::new();
        if let Some(Some((document, _))) = docs.get(idx) {
            for (i, event) in document.elements.iter().enumerate() {
                if let AxmlEvent::StartElement { attributes, .. } = event {
                    for attr in attributes {
                        let name_matches = document
                            .string(attr.name)
                            .map_or(false, |s| s == attr_name);
                        let value_matches = match &attr.typed_value {
                            TypedValue::String(si) => {
                                document.string(*si).map_or(false, |s| s == attr_value)
                            }
                            _ => false,
                        };
                        if name_matches && value_matches {
                            results.push(i as u32);
                            break;
                        }
                    }
                }
            }
        }
        results
    })
}

#[export]
pub fn xml_children(doc: u32, el: u32) -> Vec<u32> {
    XML_DOCUMENTS.with(|docs| {
        let docs = docs.borrow();
        let mut results = Vec::new();
        if let Some(Some((document, _))) = docs.get(doc as usize) {
            let start = el as usize;
            if start < document.elements.len()
                && matches!(document.elements[start], AxmlEvent::StartElement { .. })
            {
                let mut depth = 0;
                for i in start..document.elements.len() {
                    match &document.elements[i] {
                        AxmlEvent::StartElement { .. } => {
                            if depth == 1 {
                                results.push(i as u32);
                            }
                            depth += 1;
                        }
                        AxmlEvent::EndElement { .. } => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        results
    })
}

#[export]
pub fn xml_parent(doc: u32, el: u32) -> Option<u32> {
    XML_DOCUMENTS.with(|docs| {
        let docs = docs.borrow();
        if let Some(Some((document, _))) = docs.get(doc as usize) {
            let target = el as usize;
            if target < document.elements.len() {
                let mut depth = 0i32;
                for i in (0..target).rev() {
                    match &document.elements[i] {
                        AxmlEvent::EndElement { .. } => depth += 1,
                        AxmlEvent::StartElement { .. } => {
                            if depth == 0 {
                                return Some(i as u32);
                            }
                            depth -= 1;
                        }
                        _ => {}
                    }
                }
            }
        }
        None
    })
}

#[export]
pub fn xml_tag_name(doc: u32, el: u32) -> String {
    if el >= PENDING_OFFSET {
        return PENDING_ELEMENTS.with(|pe| {
            let pe = pe.borrow();
            let idx = (el - PENDING_OFFSET) as usize;
            if let Some(pending) = pe.get(idx) {
                return XML_DOCUMENTS.with(|docs| {
                    let docs = docs.borrow();
                    let doc_idx = pending.doc_idx as usize;
                    if let Some(Some((document, _))) = docs.get(doc_idx) {
                        if let Some(AxmlEvent::StartElement { name, .. }) = pending.events.first() {
                            if let Some(s) = document.string(*name) {
                                return s.to_string();
                            }
                        }
                    }
                    String::new()
                });
            }
            String::new()
        });
    }
    XML_DOCUMENTS.with(|docs| {
        let docs = docs.borrow();
        if let Some(Some((document, _))) = docs.get(doc as usize) {
            if let Some(AxmlEvent::StartElement { name, .. }) =
                document.elements.get(el as usize)
            {
                if let Some(s) = document.string(*name) {
                    return s.to_string();
                }
            }
        }
        String::new()
    })
}

#[export]
pub fn xml_get_attribute(doc: u32, el: u32, name: String) -> Option<String> {
    if el >= PENDING_OFFSET {
        return PENDING_ELEMENTS.with(|pe| {
            let pe = pe.borrow();
            let idx = (el - PENDING_OFFSET) as usize;
            if let Some(pending) = pe.get(idx) {
                return XML_DOCUMENTS.with(|docs| {
                    let docs = docs.borrow();
                    let doc_idx = pending.doc_idx as usize;
                    if let Some(Some((document, _))) = docs.get(doc_idx) {
                        if let Some(AxmlEvent::StartElement { attributes, .. }) =
                            pending.events.first()
                        {
                            return get_attr_value(document, attributes, &name);
                        }
                    }
                    None
                });
            }
            None
        });
    }
    XML_DOCUMENTS.with(|docs| {
        let docs = docs.borrow();
        if let Some(Some((document, _))) = docs.get(doc as usize) {
            if let Some(AxmlEvent::StartElement { attributes, .. }) =
                document.elements.get(el as usize)
            {
                return get_attr_value(document, attributes, &name);
            }
        }
        None
    })
}

#[export]
pub fn xml_set_attribute(doc: u32, el: u32, name: String, value: String) {
    if el >= PENDING_OFFSET {
        PENDING_ELEMENTS.with(|pe| {
            let mut pe = pe.borrow_mut();
            let idx = (el - PENDING_OFFSET) as usize;
            if idx < pe.len() {
                let doc_idx = pe[idx].doc_idx as usize;
                XML_DOCUMENTS.with(|docs| {
                    let mut docs = docs.borrow_mut();
                    if let Some(Some((document, _))) = docs.get_mut(doc_idx) {
                        let (ns, local) = resolve_namespace(document, &name);
                        let name_idx = document.string_pool.intern(local);
                        let val_idx = document.string_pool.intern(&value);
                        if let Some(AxmlEvent::StartElement { attributes, .. }) =
                            pe[idx].events.first_mut()
                        {
                            set_or_add_attr(attributes, ns, name_idx, val_idx);
                        }
                    }
                });
            }
        });
        return;
    }
    XML_DOCUMENTS.with(|docs| {
        let mut docs = docs.borrow_mut();
        if let Some(Some((document, _))) = docs.get_mut(doc as usize) {
            let (ns, local) = resolve_namespace(document, &name);
            let name_idx = document.string_pool.intern(local);
            let val_idx = document.string_pool.intern(&value);
            if let Some(AxmlEvent::StartElement { attributes, .. }) =
                document.elements.get_mut(el as usize)
            {
                set_or_add_attr(attributes, ns, name_idx, val_idx);
            }
        }
    });
}

#[export]
pub fn xml_remove_attribute(doc: u32, el: u32, name: String) {
    if el >= PENDING_OFFSET {
        PENDING_ELEMENTS.with(|pe| {
            let mut pe = pe.borrow_mut();
            let idx = (el - PENDING_OFFSET) as usize;
            if idx < pe.len() {
                let doc_idx = pe[idx].doc_idx as usize;
                XML_DOCUMENTS.with(|docs| {
                    let mut docs = docs.borrow_mut();
                    if let Some(Some((document, _))) = docs.get_mut(doc_idx) {
                        let (ns, local) = resolve_namespace(document, &name);
                        let name_idx = document.string_pool.intern(local);
                        if let Some(AxmlEvent::StartElement { attributes, .. }) =
                            pe[idx].events.first_mut()
                        {
                            attributes.retain(|a| !(a.name == name_idx && a.namespace == ns));
                        }
                    }
                });
            }
        });
        return;
    }
    XML_DOCUMENTS.with(|docs| {
        let mut docs = docs.borrow_mut();
        if let Some(Some((document, _))) = docs.get_mut(doc as usize) {
            let (ns, local) = resolve_namespace(document, &name);
            let name_idx = document.string_pool.intern(local);
            if let Some(AxmlEvent::StartElement { attributes, .. }) =
                document.elements.get_mut(el as usize)
            {
                attributes.retain(|a| !(a.name == name_idx && a.namespace == ns));
            }
        }
    });
}

#[export]
pub fn xml_create_element(doc: u32, tag: String) -> u32 {
    XML_DOCUMENTS.with(|docs| {
        let mut docs = docs.borrow_mut();
        let doc_idx = doc as usize;
        if let Some(Some((document, _))) = docs.get_mut(doc_idx) {
            let name_idx = document.string_pool.intern(&tag);
            let events = vec![
                AxmlEvent::StartElement {
                    namespace: None,
                    name: name_idx,
                    attributes: Vec::new(),
                },
                AxmlEvent::EndElement {
                    namespace: None,
                    name: name_idx,
                },
            ];
            PENDING_ELEMENTS.with(|pe| {
                let mut pe = pe.borrow_mut();
                let handle = PENDING_OFFSET + pe.len() as u32;
                pe.push(PendingElement {
                    doc_idx: doc,
                    events,
                });
                handle
            })
        } else {
            0
        }
    })
}

#[export]
pub fn xml_append_child(doc: u32, parent_el: u32, child: u32) {
    if child >= PENDING_OFFSET {
        let pending = PENDING_ELEMENTS.with(|pe| {
            let mut pe = pe.borrow_mut();
            let idx = (child - PENDING_OFFSET) as usize;
            if idx < pe.len() {
                Some(pe.remove(idx))
            } else {
                None
            }
        });
        let pending = match pending {
            Some(p) => p,
            None => return,
        };
        XML_DOCUMENTS.with(|docs| {
            let mut docs = docs.borrow_mut();
            let doc_idx = pending.doc_idx as usize;
            if let Some(Some((document, _))) = docs.get_mut(doc_idx) {
                let parent_pos = parent_el as usize;
                if parent_pos < document.elements.len() {
                    let end = element_end(document, parent_pos);
                    insert_events_at(document, end, pending.events);
                }
            }
        });
    } else {
        XML_DOCUMENTS.with(|docs| {
            let mut docs = docs.borrow_mut();
            if let Some(Some((document, _))) = docs.get_mut(doc as usize) {
                if (child as usize) < document.elements.len() {
                    let events = extract_subtree(document, child as usize);
                    let child_start = child as usize;
                    let child_count = events.len();
                    document
                        .elements
                        .drain(child_start..child_start + child_count);
                    let parent_pos = if (parent_el as usize) > child_start {
                        parent_el as usize - child_count
                    } else {
                        parent_el as usize
                    };
                    let end = element_end(document, parent_pos);
                    insert_events_at(document, end, events);
                }
            }
        });
    }
}

#[export]
pub fn xml_insert_before(doc: u32, _parent: u32, child: u32, before: u32) {
    if child >= PENDING_OFFSET {
        let pending = PENDING_ELEMENTS.with(|pe| {
            let mut pe = pe.borrow_mut();
            let idx = (child - PENDING_OFFSET) as usize;
            if idx < pe.len() {
                Some(pe.remove(idx))
            } else {
                None
            }
        });
        let pending = match pending {
            Some(p) => p,
            None => return,
        };
        XML_DOCUMENTS.with(|docs| {
            let mut docs = docs.borrow_mut();
            let doc_idx = pending.doc_idx as usize;
            if let Some(Some((document, _))) = docs.get_mut(doc_idx) {
                let before_pos = before as usize;
                if before_pos < document.elements.len() {
                    insert_events_at(document, before_pos, pending.events);
                }
            }
        });
    } else {
        XML_DOCUMENTS.with(|docs| {
            let mut docs = docs.borrow_mut();
            if let Some(Some((document, _))) = docs.get_mut(doc as usize) {
                if (child as usize) < document.elements.len() {
                    let events = extract_subtree(document, child as usize);
                    let child_start = child as usize;
                    let child_count = events.len();
                    document
                        .elements
                        .drain(child_start..child_start + child_count);
                    let before_pos = if (before as usize) > child_start {
                        before as usize - child_count
                    } else {
                        before as usize
                    };
                    insert_events_at(document, before_pos, events);
                }
            }
        });
    }
}

#[export]
pub fn xml_remove_element(doc: u32, el: u32) {
    XML_DOCUMENTS.with(|docs| {
        let mut docs = docs.borrow_mut();
        if let Some(Some((document, _))) = docs.get_mut(doc as usize) {
            if (el as usize) < document.elements.len() {
                document.remove_element(el as usize);
            }
        }
    });
}

#[export]
pub fn xml_clone_element(doc: u32, el: u32, deep: bool) -> u32 {
    XML_DOCUMENTS.with(|docs| {
        let docs = docs.borrow();
        if let Some(Some((document, _))) = docs.get(doc as usize) {
            let start = el as usize;
            if start < document.elements.len() {
                if let AxmlEvent::StartElement {
                    namespace,
                    name,
                    attributes,
                } = &document.elements[start]
                {
                    let events = if deep {
                        extract_subtree(document, start)
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
                    return PENDING_ELEMENTS.with(|pe| {
                        let mut pe = pe.borrow_mut();
                        let handle = PENDING_OFFSET + pe.len() as u32;
                        pe.push(PendingElement {
                            doc_idx: doc,
                            events,
                        });
                        handle
                    });
                }
            }
        }
        0
    })
}
