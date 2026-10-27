// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::table_bytes::{chunk_ranges, opaque_chunk};
use reseam_apk::axml::{self, AxmlAttribute, AxmlDocument};
use reseam_apk::{ResValue, ResourceScope, ResourceTable};

/// A manifest compiled from XML text, its strings in `encoding`.
pub fn make_test_axml(encoding: reseam_apk::StringEncoding) -> AxmlDocument {
    let compiled = axml::build_document(
        r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.test" android:versionCode="1" android:versionName="1.0.0"/>"#,
        None,
    )
    .expect("fixture manifest");
    let mut document = AxmlDocument::new(encoding);
    let events = document
        .adopt(compiled.string_pool(), compiled.events(), None)
        .expect("fixture manifest events");
    document.insert_events(0, events).expect("fixture manifest");
    document
}

pub fn extended_axml() -> Vec<u8> {
    let mut doc = axml::build_document(r#"<View xmlns:android="http://schemas.android.com/apk/res/android" android:id="@0x7f010000" class="CustomView" style="@0x7f020000" android:enabled="true"/>"#, None).expect("fixture XML");
    let text = doc.intern_string("content");
    let original = doc.serialize().expect("fixture bytes");
    let root = doc.root().expect("root");
    let roles = ["id", "class", "style"].map(|name| {
        doc.attributes(root)
            .iter()
            .position(|attr| doc.string(attr.name()).as_deref() == Some(name))
            .expect("special attribute") as u16
            + 1
    });
    let mut bytes = original[..8].to_vec();
    bytes[2..4].copy_from_slice(&12u16.to_le_bytes());
    bytes.extend([0x10, 0x20, 0x30, 0x40]);
    bytes.extend(opaque_chunk(0x1777, &[0x55, 0x66, 0x77, 0x88]));
    for (kind, range) in chunk_ranges(&original, 8..original.len()) {
        if !(0x100..=0x103).contains(&kind) {
            bytes.extend_from_slice(&original[range]);
            continue;
        }
        let node = &original[range];
        let mut encoded = node[..16].to_vec();
        encoded[2..4].copy_from_slice(&20u16.to_le_bytes());
        encoded[8..12].copy_from_slice(&37u32.to_le_bytes());
        encoded[12..16].copy_from_slice(&text.to_le_bytes());
        encoded.extend([0xde, 0xad, 0xbe, 0xef]);
        if kind == 0x102 {
            let count = doc.attributes(root).len();
            let mut extension = node[16..36].to_vec();
            extension[8..10].copy_from_slice(&24u16.to_le_bytes());
            extension[10..12].copy_from_slice(&24u16.to_le_bytes());
            for (role, offset) in [14, 16, 18].into_iter().enumerate() {
                extension[offset..offset + 2].copy_from_slice(&roles[role].to_le_bytes());
            }
            encoded.extend(extension);
            encoded.extend([0x11, 0x22, 0x33, 0x44]);
            for attr in node[36..36 + count * 20].as_chunks::<20>().0 {
                encoded.extend_from_slice(attr);
                encoded.extend([0xab, 0xcd, 0xef, 0x12]);
            }
            encoded.extend([0x99, 0x88, 0x77, 0x66]);
        } else {
            encoded.extend_from_slice(&node[16..]);
        }
        let len = encoded.len() as u32;
        encoded[4..8].copy_from_slice(&len.to_le_bytes());
        bytes.extend(encoded);
        if kind == 0x102 {
            let mut cdata = vec![4, 1, 16, 0, 28, 0, 0, 0];
            cdata.extend(38u32.to_le_bytes());
            cdata.extend(u32::MAX.to_le_bytes());
            cdata.extend(text.to_le_bytes());
            cdata.extend([8, 0, 0, 3]);
            cdata.extend(text.to_le_bytes());
            bytes.extend(cdata);
        }
    }
    let len = bytes.len() as u32;
    bytes[4..8].copy_from_slice(&len.to_le_bytes());
    bytes
}

pub fn view(attributes: &str, table: Option<&mut ResourceTable>) -> AxmlDocument {
    let text = format!(
        r#"<View xmlns:android="http://schemas.android.com/apk/res/android" xmlns:app="http://schemas.android.com/apk/res-auto" {attributes}/>"#
    );
    axml::build_document(&text, table.map(ResourceScope::from).as_mut()).expect("fixture XML")
}

pub fn attribute<'a>(doc: &'a AxmlDocument, element: usize, name: &str) -> &'a AxmlAttribute {
    doc.attribute_named(element, name)
        .expect("fixture attribute")
}

pub fn root_value(doc: &AxmlDocument, name: &str) -> ResValue {
    attribute(doc, doc.root().expect("fixture root"), name).value()
}
