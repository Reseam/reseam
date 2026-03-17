use stitch_apk::axml::reader::{AxmlAttribute, AxmlDocument, AxmlEvent, TypedValue};

use super::WasmState;
use super::stitch::patch::xml::Host;

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
            if let Ok(data) = document.serialize() {
                let path = path.clone();
                self.ctx().inject_file(&path, data);
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
        for doc_slot in &self.xml_documents {
            if let Some((document, _)) = doc_slot {
                if let Some(AxmlEvent::StartElement { attributes, .. }) = document.elements.get(el as usize) {
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
                }
            }
        }
        None
    }

    fn set_attribute(&mut self, el: u32, name: String, value: String) {
        for doc_slot in &mut self.xml_documents {
            if let Some((document, _)) = doc_slot {
                if (el as usize) < document.elements.len() {
                    let name_idx = document.string_pool.intern(&name);
                    let val_idx = document.string_pool.intern(&value);
                    if let Some(AxmlEvent::StartElement { attributes, .. }) = document.elements.get_mut(el as usize) {
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
                    return;
                }
            }
        }
    }

    fn remove_attribute(&mut self, el: u32, name: String) {
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
        None
    }

    fn set_text(&mut self, _el: u32, _value: String) {
    }

    fn create_element(&mut self, doc: u32, tag: String) -> u32 {
        let idx = doc as usize;
        if let Some(Some((document, _))) = self.xml_documents.get_mut(idx) {
            let events_len = document.elements.len();
            let insert_pos = if events_len > 0 { events_len - 1 } else { 0 };
            document.insert_element_before(insert_pos, &tag, Vec::new());
            return insert_pos as u32;
        }
        0
    }

    fn append_child(&mut self, _parent_el: u32, _child: u32) {
    }

    fn insert_before(&mut self, _parent: u32, _child: u32, _before: u32) {
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

    fn clone_element(&mut self, _el: u32, _deep: bool) -> u32 {
        0
    }
}
