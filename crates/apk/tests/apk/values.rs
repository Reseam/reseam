// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::common::TestResult;
use crate::common::documents::*;
use crate::common::tables::*;
use crate::common::xml_cases;
use reseam_apk::axml::{self};
use reseam_apk::resources::AttrFormats;
use reseam_apk::{ResValue, ResourceScope};

#[test]
fn test_xml_compiler_resolves_typed_resource_values() -> TestResult {
    for case in xml_cases::xml_cases()? {
        for (name, value) in case.values {
            assert_eq!(root_value(&case.doc, name), value);
        }
        for (name, kind) in case.kinds {
            assert_eq!(root_value(&case.doc, name).kind, kind);
        }
    }
    Ok(())
}

#[test]
fn attribute_formats_control_compilation_and_style_literals() -> TestResult {
    for (formats, text, kind) in xml_cases::LITERALS {
        let mut table = attribute_table(&[("value", formats)]);
        let doc = view(&format!(r#"app:value="{text}""#), Some(&mut table));
        let value = root_value(&doc, "value");
        assert_eq!(value.kind, kind, "{text}");
        if kind == ResValue::STRING {
            assert_eq!(doc.string(value.data).as_deref(), Some(text));
        }
        ResourceScope::from(&mut table).set_style_items(
            "Theme",
            Some("@0x01030005"),
            &[("value".into(), text.into())],
        )?;
        let value = table.complex_entries("style", "Theme")?.expect("bag")[0].value;
        assert_eq!(value.kind, kind);
        if kind == ResValue::STRING {
            assert_eq!(table.get_string(value.data)?.as_deref(), Some(text));
        }
    }
    for (formats, text) in [
        (AttrFormats::BOOLEAN, "12"),
        (AttrFormats::INTEGER, "true"),
        (AttrFormats::COLOR, "#éA"),
        (AttrFormats::DIMENSION, "-2147483648dp"),
    ] {
        let mut table = attribute_table(&[("value", formats)]);
        let id = table.find_resource_id("attr", "value")?.expect("ID");
        assert!(
            axml::parse_value(text, Some(id), Some(&mut ResourceScope::from(&mut table))).is_err()
        );
    }
    for text in ["true", "12", "#abc"] {
        let doc = view(&format!(r#"android:contentDescription="{text}""#), None);
        let value = root_value(&doc, "contentDescription");
        assert_eq!(value.kind, ResValue::STRING);
        assert_eq!(doc.string(value.data).as_deref(), Some(text));
    }
    Ok(())
}

#[test]
fn fractional_dimension_values_retain_precision_and_units() {
    for (text, expected, unit) in [
        ("0.5dp", 0.5, 1),
        ("0.001sp", 0.001, 2),
        ("-1.25px", -1.25, 0),
        ("8388607dp", 8_388_607.0, 1),
    ] {
        let doc = view(&format!(r#"android:padding="{text}""#), None);
        let data = root_value(&doc, "padding").data;
        let value = f64::from(data as i32 >> 8)
            / [1.0, 128.0, 32_768.0, 8_388_608.0][((data >> 4) & 3) as usize];
        assert!((value - expected).abs() < 0.000_001, "{text}");
        assert_eq!(data & 15, unit);
    }
}
