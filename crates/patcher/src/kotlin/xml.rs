use stitch_apk::axml::{AxmlAttribute, AxmlDocument, AxmlEvent, TypedValue};

use boltffi::export;

use super::{with_ctx, PENDING_ELEMENTS, XML_DOCUMENTS};

const PENDING_OFFSET: u32 = 0x8000_0000;

fn xml_file_slot_id(component_index: usize, apk_path: &str) -> String {
    format!("@file:{component_index}:{apk_path}")
}

fn parse_xml_slot_id(slot_id: &str) -> Option<(usize, &str)> {
    let rest = slot_id.strip_prefix("@file:")?;
    let (component, path) = rest.split_once(':')?;
    let component_index = component.parse().ok()?;
    Some((component_index, path))
}

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

fn resolve_namespace<'a>(document: &AxmlDocument, name: &'a str) -> (Option<u32>, &'a str) {
    if let Some(local) = name.strip_prefix("android:") {
        let ns_idx = document.elements.iter().find_map(|e| {
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

fn attr_matches(
    document: &AxmlDocument,
    attr: &AxmlAttribute,
    ns: Option<u32>,
    local: &str,
) -> bool {
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

fn parse_typed_value(
    value: &str,
    pool: &mut stitch_apk::axml::StringPool,
) -> (TypedValue, Option<u32>) {
    if value == "true" {
        return (TypedValue::Bool(true), None);
    }
    if value == "false" {
        return (TypedValue::Bool(false), None);
    }
    if value == "match_parent" || value == "fill_parent" {
        return (TypedValue::Int(-1), None);
    }
    if value == "wrap_content" {
        return (TypedValue::Int(-2), None);
    }
    if value == "@null" || value == "@empty" {
        return (TypedValue::Reference(0), None);
    }
    if let Some((data_type, data)) = stitch_apk::axml::compiler::parse_color(value) {
        return (TypedValue::Other { data_type, data }, None);
    }
    if let Some(data) = stitch_apk::axml::compiler::parse_dimension(value) {
        return (
            TypedValue::Other {
                data_type: 0x05,
                data,
            },
            None,
        );
    }
    if let Some(hex) = value.strip_prefix("0x") {
        if let Ok(v) = u32::from_str_radix(hex, 16) {
            return (TypedValue::Hex(v), None);
        }
    }
    if let Ok(v) = value.parse::<i32>() {
        return (TypedValue::Int(v), None);
    }
    if let Ok(v) = value.parse::<f32>() {
        return (
            TypedValue::Other {
                data_type: 0x04,
                data: v.to_bits(),
            },
            None,
        );
    }
    let idx = pool.intern(value);
    (TypedValue::String(idx), Some(idx))
}

fn set_or_add_attr(
    attributes: &mut Vec<AxmlAttribute>,
    ns: Option<u32>,
    name_idx: u32,
    typed_value: TypedValue,
    raw_value: Option<u32>,
) {
    for attr in attributes.iter_mut() {
        if attr.name == name_idx && attr.namespace == ns {
            attr.raw_value = raw_value;
            attr.typed_value = typed_value;
            return;
        }
    }
    attributes.push(AxmlAttribute {
        namespace: ns,
        name: name_idx,
        raw_value,
        typed_value,
    });
}

fn resolve_attr_value(document: &mut AxmlDocument, value: &str) -> (TypedValue, Option<u32>) {
    let (typed, raw) = parse_typed_value(value, &mut document.string_pool);
    if !matches!(typed, TypedValue::String(_)) {
        return (typed, raw);
    }
    if let Some(rest) = value.strip_prefix('?') {
        if let Some(attr_id) = resolve_attr_ref(rest) {
            return (
                TypedValue::Other {
                    data_type: 0x02,
                    data: attr_id,
                },
                None,
            );
        }
    }
    if let Some(rest) = value.strip_prefix('@') {
        if let Some(id) = resolve_xml_resource_ref(rest) {
            return (TypedValue::Reference(id), None);
        }
    }
    (typed, raw)
}

fn resolve_attr_ref(s: &str) -> Option<u32> {
    if let Some(name) = s.strip_prefix("android:attr/") {
        return stitch_apk::axml::compiler::android_attr_res_id(name);
    }
    let name = s.strip_prefix("attr/").unwrap_or(s);
    with_ctx(|ctx| ctx.find_resource_id("attr", name))
}

fn resolve_xml_resource_ref(s: &str) -> Option<u32> {
    let (namespace, type_name, entry_name, create_id) = parse_xml_resource_ref(s)?;
    match namespace {
        Some("android") if type_name == "attr" => {
            stitch_apk::axml::compiler::android_attr_res_id(entry_name)
        }
        Some(_) => None,
        None if create_id && type_name == "id" => {
            with_ctx(|ctx| ctx.resources_mut()?.ensure_id(entry_name))
        }
        None => with_ctx(|ctx| ctx.find_resource_id(type_name, entry_name)),
    }
}

fn parse_xml_resource_ref(s: &str) -> Option<(Option<&str>, &str, &str, bool)> {
    let create_id = s.starts_with("+id/");
    let s = s.strip_prefix('+').unwrap_or(s);
    let slash = s.find('/')?;
    let (type_part, entry_name) = (&s[..slash], &s[slash + 1..]);
    let (namespace, type_name) = if let Some(colon) = type_part.find(':') {
        (Some(&type_part[..colon]), &type_part[colon + 1..])
    } else {
        (None, type_part)
    };
    if type_name.is_empty() || entry_name.is_empty() {
        return None;
    }
    Some((namespace, type_name, entry_name, create_id))
}

fn insert_events_at(doc: &mut AxmlDocument, pos: usize, events: Vec<AxmlEvent>) {
    for (j, event) in events.into_iter().enumerate() {
        doc.elements.insert(pos + j, event);
    }
}

#[export]
pub fn xml_open(apk_path: String) -> Option<u32> {
    xml_open_in_component("base".to_string(), apk_path)
}

#[export]
pub fn xml_open_in_component(component: String, apk_path: String) -> Option<u32> {
    with_ctx(|ctx| {
        let component_index = ctx.component_index(&component)?;
        let data = ctx.read_file_from_component(component_index, &apk_path)?;
        let doc = AxmlDocument::parse(&data).ok()?;
        let slot_id = xml_file_slot_id(component_index, &apk_path);
        XML_DOCUMENTS.with(|docs| {
            let mut docs = docs.borrow_mut();
            let handle = docs.len() as u32;
            docs.push(Some((doc, slot_id)));
            Some(handle)
        })
    })
}

#[export]
pub fn xml_close(doc: u32) {
    XML_DOCUMENTS.with(|docs| {
        let mut docs = docs.borrow_mut();
        let idx = doc as usize;
        let mut clear_slot = true;
        if let Some(Some((document, path))) = docs.get(idx) {
            match document.serialize() {
                Ok(data) => {
                    let path = path.clone();
                    with_ctx(|ctx| {
                        if let Some((component_index, apk_path)) = parse_xml_slot_id(&path) {
                            ctx.inject_file_into_component(component_index, apk_path, data);
                        } else if let Some(component_index) =
                            path.strip_prefix("@manifest:").and_then(|s| s.parse::<usize>().ok())
                        {
                            if let Some(manifest) = ctx.component_manifest_mut(component_index) {
                                *manifest = document.clone();
                            }
                        }
                    });
                }
                Err(e) => {
                    let path = path.clone();
                    clear_slot = false;
                    with_ctx(|ctx| {
                        ctx.log()
                            .warn(format!("xml close: failed to serialize {path}: {e}"))
                    });
                }
            }
        }
        if clear_slot {
            if let Some(slot) = docs.get_mut(idx) {
                *slot = None;
            }
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
                    if get_attr_value(document, attributes, &attr_name).as_deref()
                        == Some(&attr_value)
                    {
                        results.push(i as u32);
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
            if let Some(AxmlEvent::StartElement { name, .. }) = document.elements.get(el as usize) {
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
                        let (typed_value, raw_value) = resolve_attr_value(document, &value);
                        if let Some(AxmlEvent::StartElement { attributes, .. }) =
                            pe[idx].events.first_mut()
                        {
                            set_or_add_attr(attributes, ns, name_idx, typed_value, raw_value);
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
            let (typed_value, raw_value) = resolve_attr_value(document, &value);
            if let Some(AxmlEvent::StartElement { attributes, .. }) =
                document.elements.get_mut(el as usize)
            {
                set_or_add_attr(attributes, ns, name_idx, typed_value, raw_value);
            }
        }
    });
}

#[export]
pub fn xml_set_attribute_int(doc: u32, el: u32, name: String, value: i32) {
    set_attribute_typed(doc, el, &name, TypedValue::Int(value));
}

#[export]
pub fn xml_set_attribute_bool(doc: u32, el: u32, name: String, value: bool) {
    set_attribute_typed(doc, el, &name, TypedValue::Bool(value));
}

#[export]
pub fn xml_set_attribute_ref(doc: u32, el: u32, name: String, res_id: u32) {
    set_attribute_typed(doc, el, &name, TypedValue::Reference(res_id));
}

fn set_attribute_typed(doc: u32, el: u32, name: &str, typed_value: TypedValue) {
    if el >= PENDING_OFFSET {
        PENDING_ELEMENTS.with(|pe| {
            let mut pe = pe.borrow_mut();
            let idx = (el - PENDING_OFFSET) as usize;
            if idx < pe.len() {
                let doc_idx = pe[idx].doc_idx as usize;
                XML_DOCUMENTS.with(|docs| {
                    let mut docs = docs.borrow_mut();
                    if let Some(Some((document, _))) = docs.get_mut(doc_idx) {
                        let (ns, local) = resolve_namespace(document, name);
                        let name_idx = document.string_pool.intern(local);
                        if let Some(AxmlEvent::StartElement { attributes, .. }) =
                            pe[idx].events.first_mut()
                        {
                            set_or_add_attr(attributes, ns, name_idx, typed_value, None);
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
            let (ns, local) = resolve_namespace(document, name);
            let name_idx = document.string_pool.intern(local);
            if let Some(AxmlEvent::StartElement { attributes, .. }) =
                document.elements.get_mut(el as usize)
            {
                set_or_add_attr(attributes, ns, name_idx, typed_value, None);
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

#[cfg(test)]
mod tests {
    use super::*;
    use stitch_apk::axml::StringPool;

    fn with_test_doc(f: impl FnOnce(u32)) -> AxmlDocument {
        let doc = AxmlDocument {
            string_pool: StringPool {
                strings: vec![
                    ANDROID_NS_URI.to_string(),
                    "android".to_string(),
                    "LinearLayout".to_string(),
                ],
                is_utf8: true,
            },
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
        XML_DOCUMENTS.with(|docs| {
            let mut docs = docs.borrow_mut();
            docs.clear();
            docs.push(Some((doc, "res/layout/test.xml".to_string())));
        });
        f(0);
        let document = XML_DOCUMENTS.with(|docs| {
            let docs = docs.borrow();
            docs[0].as_ref().expect("test document").0.clone()
        });
        XML_DOCUMENTS.with(|docs| docs.borrow_mut().clear());
        document
    }

    #[test]
    fn xml_set_attribute_preserves_typed_values() {
        let document = with_test_doc(|doc| {
            xml_set_attribute(doc, 1, "android:padding".to_string(), "16dp".to_string());
            xml_set_attribute(doc, 1, "android:alpha".to_string(), "0.5".to_string());
            xml_set_attribute(
                doc,
                1,
                "android:textColor".to_string(),
                "?android:attr/textColor".to_string(),
            );
        });
        let attributes = match &document.elements[1] {
            AxmlEvent::StartElement { attributes, .. } => attributes,
            _ => panic!("expected start element"),
        };
        let attr = |name: &str| {
            attributes
                .iter()
                .find(|attr| document.string(attr.name) == Some(name))
                .expect("attribute present")
        };

        assert!(matches!(
            attr("padding").typed_value,
            TypedValue::Other {
                data_type: 0x05,
                ..
            }
        ));
        assert!(matches!(
            attr("alpha").typed_value,
            TypedValue::Other {
                data_type: 0x04,
                ..
            }
        ));
        assert!(matches!(
            attr("textColor").typed_value,
            TypedValue::Other { data_type: 0x02, data }
                if data == stitch_apk::axml::compiler::android_attr_res_id("textColor").unwrap()
        ));
    }
}
