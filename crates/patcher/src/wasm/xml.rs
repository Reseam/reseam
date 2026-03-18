use stitch_apk::axml::reader::{AxmlAttribute, AxmlDocument, AxmlEvent, TypedValue};

use super::WasmState;
use super::stitch::patch::xml::Host;

const PENDING_OFFSET: u32 = 0x8000_0000;

pub(super) struct PendingElement {
    doc_idx: u32,
    events: Vec<AxmlEvent>,
}

fn find_doc_for_element_mut<'a>(
    docs: &'a mut [Option<(AxmlDocument, String)>],
    el: u32,
) -> Option<(usize, &'a mut AxmlDocument)> {
    let pos = el as usize;
    for (i, slot) in docs.iter_mut().enumerate() {
        if let Some((doc, _)) = slot {
            if pos < doc.elements.len() {
                if matches!(doc.elements[pos], AxmlEvent::StartElement { .. }) {
                    return Some((i, doc));
                }
            }
        }
    }
    None
}

fn element_end(doc: &AxmlDocument, start: usize) -> usize {
    doc.find_end_element(start).unwrap_or(start)
}

fn extract_subtree(doc: &AxmlDocument, start: usize) -> Vec<AxmlEvent> {
    let end = element_end(doc, start);
    doc.elements[start..=end].to_vec()
}

impl WasmState {
    fn take_pending(&mut self, handle: u32) -> Option<PendingElement> {
        let idx = (handle - PENDING_OFFSET) as usize;
        if idx < self.pending_elements.len() {
            Some(self.pending_elements.remove(idx))
        } else {
            None
        }
    }

    fn insert_events_at(doc: &mut AxmlDocument, pos: usize, events: Vec<AxmlEvent>) {
        for (j, event) in events.into_iter().enumerate() {
            doc.elements.insert(pos + j, event);
        }
    }
}

impl Host for WasmState {
    fn open(&mut self, apk_path: String) -> Option<u32> {
        let ctx = self.ctx();
        let data = ctx.apk().read_entry(&apk_path).ok()?;
        let doc = AxmlDocument::parse(&data).ok()?;
        let handle = self.xml_documents.len() as u32;
        self.xml_documents.push(Some((doc, apk_path)));
        Some(handle)
    }

    fn close(&mut self, doc: u32) {
        let idx = doc as usize;
        if let Some(Some((document, path))) = self.xml_documents.get(idx) {
            match document.serialize() {
                Ok(data) => {
                    let path = path.clone();
                    self.ctx().inject_file(&path, data);
                }
                Err(e) => {
                    let path = path.clone();
                    self.ctx().log().warn(format!(
                        "xml close: failed to serialize {path}: {e}"
                    ));
                }
            }
        }
        if let Some(slot) = self.xml_documents.get_mut(idx) {
            *slot = None;
        }
    }

    fn root(&mut self, doc: u32) -> u32 {
        let idx = doc as usize;
        if let Some(Some((document, _))) = self.xml_documents.get(idx) {
            for (i, event) in document.elements.iter().enumerate() {
                if matches!(event, AxmlEvent::StartElement { .. }) {
                    return i as u32;
                }
            }
        }
        0
    }

    fn find_by_tag(&mut self, doc: u32, tag: String) -> Vec<u32> {
        let idx = doc as usize;
        let mut results = Vec::new();
        if let Some(Some((document, _))) = self.xml_documents.get(idx) {
            for (i, event) in document.elements.iter().enumerate() {
                if let AxmlEvent::StartElement { name, .. } = event {
                    if document.string(*name).map_or(false, |s| s == tag) {
                        results.push(i as u32);
                    }
                }
            }
        }
        results
    }

    fn find_by_attribute(&mut self, doc: u32, attr_name: String, attr_value: String) -> Vec<u32> {
        let idx = doc as usize;
        let mut results = Vec::new();
        if let Some(Some((document, _))) = self.xml_documents.get(idx) {
            for (i, event) in document.elements.iter().enumerate() {
                if let AxmlEvent::StartElement { attributes, .. } = event {
                    for attr in attributes {
                        let name_matches = document.string(attr.name)
                            .map_or(false, |s| s == attr_name);
                        let value_matches = match &attr.typed_value {
                            TypedValue::String(si) => document.string(*si)
                                .map_or(false, |s| s == attr_value),
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
    }

    fn children(&mut self, el: u32) -> Vec<u32> {
        let mut results = Vec::new();
        for doc_slot in &self.xml_documents {
            if let Some((document, _)) = doc_slot {
                let start = el as usize;
                if start >= document.elements.len() {
                    continue;
                }
                if !matches!(document.elements[start], AxmlEvent::StartElement { .. }) {
                    continue;
                }
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
                if !results.is_empty() {
                    return results;
                }
            }
        }
        results
    }

    fn parent(&mut self, el: u32) -> Option<u32> {
        for doc_slot in &self.xml_documents {
            if let Some((document, _)) = doc_slot {
                let target = el as usize;
                if target >= document.elements.len() {
                    continue;
                }
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
    }

    fn tag_name(&mut self, el: u32) -> String {
        if el >= PENDING_OFFSET {
            let idx = (el - PENDING_OFFSET) as usize;
            if let Some(pending) = self.pending_elements.get(idx) {
                let doc_idx = pending.doc_idx as usize;
                if let Some(Some((document, _))) = self.xml_documents.get(doc_idx) {
                    if let Some(AxmlEvent::StartElement { name, .. }) = pending.events.first() {
                        if let Some(s) = document.string(*name) {
                            return s.to_string();
                        }
                    }
                }
            }
            return String::new();
        }
        for doc_slot in &self.xml_documents {
            if let Some((document, _)) = doc_slot {
                if let Some(AxmlEvent::StartElement { name, .. }) = document.elements.get(el as usize) {
                    if let Some(s) = document.string(*name) {
                        return s.to_string();
                    }
                }
            }
        }
        String::new()
    }

    fn get_attribute(&mut self, el: u32, name: String) -> Option<String> {
        if el >= PENDING_OFFSET {
            let idx = (el - PENDING_OFFSET) as usize;
            if let Some(pending) = self.pending_elements.get(idx) {
                let doc_idx = pending.doc_idx as usize;
                if let Some(Some((document, _))) = self.xml_documents.get(doc_idx) {
                    if let Some(AxmlEvent::StartElement { attributes, .. }) = pending.events.first() {
                        return get_attr_value(document, attributes, &name);
                    }
                }
            }
            return None;
        }
        for doc_slot in &self.xml_documents {
            if let Some((document, _)) = doc_slot {
                if let Some(AxmlEvent::StartElement { attributes, .. }) = document.elements.get(el as usize) {
                    return get_attr_value(document, attributes, &name);
                }
            }
        }
        None
    }

    fn set_attribute(&mut self, el: u32, name: String, value: String) {
        if el >= PENDING_OFFSET {
            let idx = (el - PENDING_OFFSET) as usize;
            if idx < self.pending_elements.len() {
                let doc_idx = self.pending_elements[idx].doc_idx as usize;
                if let Some(Some((document, _))) = self.xml_documents.get_mut(doc_idx) {
                    let name_idx = document.string_pool.intern(&name);
                    let val_idx = document.string_pool.intern(&value);
                    if let Some(AxmlEvent::StartElement { attributes, .. }) = self.pending_elements[idx].events.first_mut() {
                        set_or_add_attr(attributes, name_idx, val_idx);
                    }
                }
            }
            return;
        }
        for doc_slot in &mut self.xml_documents {
            if let Some((document, _)) = doc_slot {
                if (el as usize) < document.elements.len() {
                    let name_idx = document.string_pool.intern(&name);
                    let val_idx = document.string_pool.intern(&value);
                    if let Some(AxmlEvent::StartElement { attributes, .. }) = document.elements.get_mut(el as usize) {
                        set_or_add_attr(attributes, name_idx, val_idx);
                    }
                    return;
                }
            }
        }
    }

    fn remove_attribute(&mut self, el: u32, name: String) {
        if el >= PENDING_OFFSET {
            let idx = (el - PENDING_OFFSET) as usize;
            if idx < self.pending_elements.len() {
                let doc_idx = self.pending_elements[idx].doc_idx as usize;
                if let Some(Some((document, _))) = self.xml_documents.get_mut(doc_idx) {
                    let name_idx = document.string_pool.intern(&name);
                    if let Some(AxmlEvent::StartElement { attributes, .. }) = self.pending_elements[idx].events.first_mut() {
                        attributes.retain(|a| a.name != name_idx);
                    }
                }
            }
            return;
        }
        for doc_slot in &mut self.xml_documents {
            if let Some((document, _)) = doc_slot {
                if (el as usize) < document.elements.len() {
                    let name_idx = document.string_pool.intern(&name);
                    if let Some(AxmlEvent::StartElement { attributes, .. }) = document.elements.get_mut(el as usize) {
                        attributes.retain(|a| a.name != name_idx);
                    }
                    return;
                }
            }
        }
    }

    fn get_text(&mut self, _el: u32) -> Option<String> {
        // AXML binary format doesn't have text nodes — Android XML uses attributes for all values
        None
    }

    fn set_text(&mut self, _el: u32, _value: String) {
        // AXML binary format doesn't have text nodes — Android XML uses attributes for all values
    }

    fn create_element(&mut self, doc: u32, tag: String) -> u32 {
        let doc_idx = doc as usize;
        if let Some(Some((document, _))) = self.xml_documents.get_mut(doc_idx) {
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
            let handle = PENDING_OFFSET + self.pending_elements.len() as u32;
            self.pending_elements.push(PendingElement {
                doc_idx: doc,
                events,
            });
            return handle;
        }
        0
    }

    fn append_child(&mut self, parent_el: u32, child: u32) {
        if child >= PENDING_OFFSET {
            let pending = match self.take_pending(child) {
                Some(p) => p,
                None => return,
            };
            let doc_idx = pending.doc_idx as usize;
            if let Some(Some((document, _))) = self.xml_documents.get_mut(doc_idx) {
                let parent_pos = parent_el as usize;
                if parent_pos >= document.elements.len() {
                    return;
                }
                let end = element_end(document, parent_pos);
                WasmState::insert_events_at(document, end, pending.events);
            }
        } else {
            // Move an existing element within the document
            let events = {
                let doc = self.xml_documents.iter()
                    .filter_map(|s| s.as_ref())
                    .find(|(d, _)| (child as usize) < d.elements.len());
                match doc {
                    Some((document, _)) => extract_subtree(document, child as usize),
                    None => return,
                }
            };
            if let Some((_, document)) = find_doc_for_element_mut(&mut self.xml_documents, parent_el) {
                // Remove old location first
                let child_start = child as usize;
                let child_count = events.len();
                document.elements.drain(child_start..child_start + child_count);
                // Recalculate parent end after removal
                let parent_pos = if (parent_el as usize) > child_start {
                    parent_el as usize - child_count
                } else {
                    parent_el as usize
                };
                let end = element_end(document, parent_pos);
                WasmState::insert_events_at(document, end, events);
            }
        }
    }

    fn insert_before(&mut self, _parent: u32, child: u32, before: u32) {
        if child >= PENDING_OFFSET {
            let pending = match self.take_pending(child) {
                Some(p) => p,
                None => return,
            };
            let doc_idx = pending.doc_idx as usize;
            if let Some(Some((document, _))) = self.xml_documents.get_mut(doc_idx) {
                let before_pos = before as usize;
                if before_pos >= document.elements.len() {
                    return;
                }
                WasmState::insert_events_at(document, before_pos, pending.events);
            }
        } else {
            // Move existing element before another
            let events = {
                let doc = self.xml_documents.iter()
                    .filter_map(|s| s.as_ref())
                    .find(|(d, _)| (child as usize) < d.elements.len());
                match doc {
                    Some((document, _)) => extract_subtree(document, child as usize),
                    None => return,
                }
            };
            if let Some((_, document)) = find_doc_for_element_mut(&mut self.xml_documents, before) {
                let child_start = child as usize;
                let child_count = events.len();
                document.elements.drain(child_start..child_start + child_count);
                let before_pos = if (before as usize) > child_start {
                    before as usize - child_count
                } else {
                    before as usize
                };
                WasmState::insert_events_at(document, before_pos, events);
            }
        }
    }

    fn remove_element(&mut self, el: u32) {
        for doc_slot in &mut self.xml_documents {
            if let Some((document, _)) = doc_slot {
                if (el as usize) < document.elements.len() {
                    document.remove_element(el as usize);
                    return;
                }
            }
        }
    }

    fn clone_element(&mut self, el: u32, deep: bool) -> u32 {
        for (doc_idx, doc_slot) in self.xml_documents.iter().enumerate() {
            if let Some((document, _)) = doc_slot {
                let start = el as usize;
                if start >= document.elements.len() {
                    continue;
                }
                if !matches!(document.elements[start], AxmlEvent::StartElement { .. }) {
                    continue;
                }
                let events = if deep {
                    extract_subtree(document, start)
                } else {
                    if let AxmlEvent::StartElement { namespace, name, attributes } = &document.elements[start] {
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
                    } else {
                        continue;
                    }
                };
                let handle = PENDING_OFFSET + self.pending_elements.len() as u32;
                self.pending_elements.push(PendingElement {
                    doc_idx: doc_idx as u32,
                    events,
                });
                return handle;
            }
        }
        0
    }
}

fn get_attr_value(document: &AxmlDocument, attributes: &[AxmlAttribute], name: &str) -> Option<String> {
    for attr in attributes {
        if document.string(attr.name).map_or(false, |s| s == name) {
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

fn set_or_add_attr(attributes: &mut Vec<AxmlAttribute>, name_idx: u32, val_idx: u32) {
    for attr in attributes.iter_mut() {
        if attr.name == name_idx {
            attr.raw_value = Some(val_idx);
            attr.typed_value = TypedValue::String(val_idx);
            return;
        }
    }
    attributes.push(AxmlAttribute {
        namespace: None,
        name: name_idx,
        raw_value: Some(val_idx),
        typed_value: TypedValue::String(val_idx),
    });
}
