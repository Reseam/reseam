// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::documents::view;
use super::tables::{make_attr_arsc, make_test_arsc};
use reseam_apk::ResValue;
use reseam_apk::resources::AttrFormats;

pub struct XmlCase {
    pub doc: reseam_apk::axml::AxmlDocument,
    pub values: Vec<(&'static str, ResValue)>,
    pub kinds: Vec<(&'static str, u8)>,
}

pub fn xml_cases() -> reseam_apk::Result<Vec<XmlCase>> {
    let mut table = make_test_arsc();
    let title = table
        .add_resource("attr", "titleText", ResValue::new(0, 0))?
        .expect("fixture ID");
    let doc = view(
        r#"android:text="@string/hello" android:id="@+id/title" android:theme="?attr/titleText" android:textColor="?android:attr/textColor" android:background="@android:color/white" android:padding="16dp" android:alpha="0.5""#,
        Some(&mut table),
    );
    Ok(vec![
        XmlCase {
            doc,
            values: vec![
                (
                    "text",
                    ResValue::reference(table.find_resource_id("string", "hello")?.expect("ID")),
                ),
                (
                    "id",
                    ResValue::reference(table.find_resource_id("id", "title")?.expect("ID")),
                ),
                ("theme", ResValue::attribute(title)),
                (
                    "textColor",
                    ResValue::attribute(
                        reseam_apk::axml::android_attr_res_id("textColor").expect("ID"),
                    ),
                ),
                (
                    "background",
                    ResValue::reference(
                        reseam_apk::axml::android_res_id("color", "white").expect("ID"),
                    ),
                ),
            ],
            kinds: vec![("padding", ResValue::DIMENSION), ("alpha", ResValue::FLOAT)],
        },
        XmlCase {
            doc: view(
                r#"android:scaleType="center" android:gravity="center_vertical | end" android:contentDescription="center" app:mode="wide" app:sides="top|bottom""#,
                Some(&mut make_attr_arsc()),
            ),
            values: vec![
                ("scaleType", ResValue::int(5)),
                ("gravity", ResValue::hex(0x0080_0015)),
                ("mode", ResValue::int(2)),
                ("sides", ResValue::hex(3)),
            ],
            kinds: vec![("contentDescription", ResValue::STRING)],
        },
    ])
}

pub const LITERALS: [(AttrFormats, &str, u8); 13] = [
    (AttrFormats::STRING, "true", ResValue::STRING),
    (AttrFormats::STRING, "12", ResValue::STRING),
    (AttrFormats::STRING, "1.5", ResValue::STRING),
    (AttrFormats::STRING, "#abc", ResValue::STRING),
    (AttrFormats::STRING, "16dp", ResValue::STRING),
    (AttrFormats::STRING, "wrap_content", ResValue::STRING),
    (AttrFormats::INTEGER, "12", ResValue::INT_DEC),
    (AttrFormats::INTEGER, "0xff", ResValue::INT_HEX),
    (AttrFormats::FLOAT, "2", ResValue::FLOAT),
    (AttrFormats::BOOLEAN, "false", ResValue::INT_BOOLEAN),
    (AttrFormats::COLOR, "#abc", ResValue::INT_COLOR_RGB8),
    (AttrFormats::DIMENSION, "16dp", ResValue::DIMENSION),
    (AttrFormats::FRACTION, "50%", ResValue::FRACTION),
];
