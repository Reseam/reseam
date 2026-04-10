use crate::axml::reader::{AxmlAttribute, AxmlDocument, AxmlEvent, TypedValue};
use crate::axml::string_pool::StringPool;
use crate::error::{invalid, Result};
use crate::resources::ResourceTable;

const ANDROID_NS: &str = "http://schemas.android.com/apk/res/android";
const APP_NS: &str = "http://schemas.android.com/apk/res-auto";

pub fn compile_xml(text: &str) -> Result<Vec<u8>> {
    compile_xml_with_resources(text, None)
}

pub fn compile_xml_with_resources(
    text: &str,
    resources: Option<&mut ResourceTable>,
) -> Result<Vec<u8>> {
    let doc = build_axml_document_with_resources(text, resources)?;
    doc.serialize()
}

pub fn build_axml_document(text: &str) -> Result<AxmlDocument> {
    build_axml_document_with_resources(text, None)
}

pub fn build_axml_document_with_resources(
    text: &str,
    mut resources: Option<&mut ResourceTable>,
) -> Result<AxmlDocument> {
    let mut pool = StringPool {
        strings: Vec::new(),
        is_utf8: true,
    };
    let mut resource_ids: Vec<u32> = Vec::new();
    let mut events: Vec<AxmlEvent> = Vec::new();

    // First pass: collect all android: attribute names that need resource IDs.
    // These must be interned first (indices 0..N) to align with the resource_ids array.
    let mut attr_names_with_res_id: Vec<(String, u32)> = Vec::new();
    {
        let mut reader = quick_xml::Reader::from_str(text);
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Eof) => break,
                Ok(quick_xml::events::Event::Start(ref e))
                | Ok(quick_xml::events::Event::Empty(ref e)) => {
                    for attr in e.attributes() {
                        let attr = attr.map_err(|e| {
                            invalid("axml compiler", format!("invalid XML attribute: {e}"))
                        })?;
                        let key = std::str::from_utf8(attr.key.as_ref()).map_err(|e| {
                            invalid(
                                "axml compiler",
                                format!("invalid UTF-8 in attribute key: {e}"),
                            )
                        })?;
                        if let Some(local) = key.strip_prefix("android:") {
                            if let Some(res_id) = android_attr_res_id(local) {
                                if !attr_names_with_res_id.iter().any(|(n, _)| n == local) {
                                    attr_names_with_res_id.push((local.to_string(), res_id));
                                }
                            }
                        }
                    }
                }
                Err(e) => return Err(invalid("axml compiler", format!("XML parse error: {e}"))),
                _ => {}
            }
            buf.clear();
        }
    }

    // Intern attribute names with resource IDs first
    for (name, res_id) in &attr_names_with_res_id {
        let idx = pool.intern(name);
        // resource_ids[idx] = res_id
        while resource_ids.len() <= idx as usize {
            resource_ids.push(0);
        }
        resource_ids[idx as usize] = *res_id;
    }

    // Second pass: build events
    let mut reader = quick_xml::Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut ns_emitted = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(quick_xml::events::Event::Eof) => break,
            Ok(quick_xml::events::Event::Start(ref e)) => {
                emit_start_element(
                    e,
                    &mut pool,
                    &resource_ids,
                    &mut events,
                    &mut ns_emitted,
                    false,
                    resources.as_deref_mut(),
                )?;
            }
            Ok(quick_xml::events::Event::Empty(ref e)) => {
                emit_start_element(
                    e,
                    &mut pool,
                    &resource_ids,
                    &mut events,
                    &mut ns_emitted,
                    true,
                    resources.as_deref_mut(),
                )?;
            }
            Ok(quick_xml::events::Event::End(ref e)) => {
                let name_bytes = e.name();
                let raw_name = std::str::from_utf8(name_bytes.as_ref()).unwrap_or("");
                let local = raw_name.split(':').last().unwrap_or(raw_name);
                let name_idx = pool.intern(local);
                events.push(AxmlEvent::EndElement {
                    namespace: None,
                    name: name_idx,
                });
            }
            Err(e) => return Err(invalid("axml compiler", format!("XML parse error: {e}"))),
            _ => {}
        }
        buf.clear();
    }

    // Add EndNamespace if we emitted StartNamespace
    if ns_emitted {
        let prefix_idx = pool.intern("android");
        let uri_idx = pool.intern(ANDROID_NS);
        events.push(AxmlEvent::EndNamespace {
            prefix: Some(prefix_idx),
            uri: uri_idx,
        });
        // Check if app namespace was used
        if pool.strings.iter().any(|s| s == APP_NS) {
            let app_prefix = pool.intern("app");
            let app_uri = pool.intern(APP_NS);
            events.push(AxmlEvent::EndNamespace {
                prefix: Some(app_prefix),
                uri: app_uri,
            });
        }
    }

    Ok(AxmlDocument {
        string_pool: pool,
        resource_ids,
        elements: events,
    })
}

fn emit_start_element(
    e: &quick_xml::events::BytesStart<'_>,
    pool: &mut StringPool,
    resource_ids: &[u32],
    events: &mut Vec<AxmlEvent>,
    ns_emitted: &mut bool,
    is_empty: bool,
    mut resources: Option<&mut ResourceTable>,
) -> Result<()> {
    // Emit namespace declarations on first element
    if !*ns_emitted {
        let mut has_android_ns = false;
        let mut has_app_ns = false;
        for attr in e.attributes() {
            let attr =
                attr.map_err(|e| invalid("axml compiler", format!("invalid XML attribute: {e}")))?;
            let key = std::str::from_utf8(attr.key.as_ref()).map_err(|e| {
                invalid(
                    "axml compiler",
                    format!("invalid UTF-8 in attribute key: {e}"),
                )
            })?;
            has_android_ns |= key == "xmlns:android" || key.starts_with("android:");
            has_app_ns |= key == "xmlns:app" || key.starts_with("app:");
        }
        if has_android_ns {
            let prefix_idx = pool.intern("android");
            let uri_idx = pool.intern(ANDROID_NS);
            events.push(AxmlEvent::StartNamespace {
                prefix: Some(prefix_idx),
                uri: uri_idx,
            });
        }
        if has_app_ns {
            let app_prefix = pool.intern("app");
            let app_uri = pool.intern(APP_NS);
            events.push(AxmlEvent::StartNamespace {
                prefix: Some(app_prefix),
                uri: app_uri,
            });
        }
        *ns_emitted = true;
    }

    let name_bytes = e.name();
    let raw_name = std::str::from_utf8(name_bytes.as_ref()).unwrap_or("");
    let local = raw_name.split(':').last().unwrap_or(raw_name);
    let name_idx = pool.intern(local);

    let mut attributes = Vec::new();
    for attr in e.attributes() {
        let attr =
            attr.map_err(|e| invalid("axml compiler", format!("invalid XML attribute: {e}")))?;
        let key = std::str::from_utf8(attr.key.as_ref()).map_err(|e| {
            invalid(
                "axml compiler",
                format!("invalid UTF-8 in attribute key: {e}"),
            )
        })?;
        // Skip xmlns declarations — handled as namespace events
        if key.starts_with("xmlns:") || key == "xmlns" {
            continue;
        }

        let value = attr
            .unescape_value()
            .map_err(|e| invalid("axml compiler", format!("invalid XML attribute value: {e}")))?
            .to_string();
        let (namespace, attr_local) = if let Some(local) = key.strip_prefix("android:") {
            let uri_idx = pool.intern(ANDROID_NS);
            (Some(uri_idx), local)
        } else if let Some(local) = key.strip_prefix("app:") {
            let uri_idx = pool.intern(APP_NS);
            (Some(uri_idx), local)
        } else {
            (None, key)
        };

        let attr_name_idx = pool.intern(attr_local);
        let (typed_value, raw_value) = parse_attr_value(&value, pool, resources.as_deref_mut());

        attributes.push(AxmlAttribute {
            namespace,
            name: attr_name_idx,
            raw_value,
            typed_value,
        });
    }

    // Sort attributes: android: namespace first (by resource ID), then others
    attributes.sort_by(|a, b| {
        let a_res = if (a.name as usize) < resource_ids.len() {
            resource_ids[a.name as usize]
        } else {
            u32::MAX
        };
        let b_res = if (b.name as usize) < resource_ids.len() {
            resource_ids[b.name as usize]
        } else {
            u32::MAX
        };
        a_res.cmp(&b_res)
    });

    events.push(AxmlEvent::StartElement {
        namespace: None,
        name: name_idx,
        attributes,
    });

    if is_empty {
        events.push(AxmlEvent::EndElement {
            namespace: None,
            name: name_idx,
        });
    }

    Ok(())
}

fn parse_attr_value(
    value: &str,
    pool: &mut StringPool,
    mut resources: Option<&mut ResourceTable>,
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

    if let Some(color) = parse_color(value) {
        return (
            TypedValue::Other {
                data_type: color.0,
                data: color.1,
            },
            None,
        );
    }

    if let Some(dim) = parse_dimension(value) {
        return (
            TypedValue::Other {
                data_type: 0x05,
                data: dim,
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

    if let Some(rest) = value.strip_prefix('?') {
        if let Some(attr_id) = resolve_attribute_ref(rest, resources.as_deref()) {
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
        if let Some(id) = resolve_resource_ref(rest, resources.as_deref_mut()) {
            return (TypedValue::Reference(id), None);
        }
    }

    let idx = pool.intern(value);
    (TypedValue::String(idx), Some(idx))
}

fn resolve_resource_ref(s: &str, resources: Option<&mut ResourceTable>) -> Option<u32> {
    let (namespace, type_name, entry_name, create_id) = parse_resource_ref(s)?;
    match namespace {
        Some("android") if type_name == "attr" => android_attr_res_id(entry_name),
        Some(_) => None,
        None => {
            let res = resources?;
            if create_id && type_name == "id" {
                res.ensure_id(entry_name)
            } else {
                res.find_resource_id(type_name, entry_name)
            }
        }
    }
}

fn resolve_attribute_ref(s: &str, resources: Option<&ResourceTable>) -> Option<u32> {
    if let Some(name) = s.strip_prefix("android:attr/") {
        return android_attr_res_id(name);
    }
    let name = s.strip_prefix("attr/").unwrap_or(s);
    resources?.find_resource_id("attr", name)
}

fn parse_resource_ref(s: &str) -> Option<(Option<&str>, &str, &str, bool)> {
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

pub fn parse_color(s: &str) -> Option<(u8, u32)> {
    let hex = s.strip_prefix('#')?;
    match hex.len() {
        // #RGB → #FFRRGGBB
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
            let color = 0xFF000000
                | ((r as u32 * 0x11) << 16)
                | ((g as u32 * 0x11) << 8)
                | (b as u32 * 0x11);
            Some((0x1d, color)) // TYPE_INT_COLOR_RGB8
        }
        // #ARGB → #AARRGGBB
        4 => {
            let a = u8::from_str_radix(&hex[0..1], 16).ok()?;
            let r = u8::from_str_radix(&hex[1..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..3], 16).ok()?;
            let b = u8::from_str_radix(&hex[3..4], 16).ok()?;
            let color = ((a as u32 * 0x11) << 24)
                | ((r as u32 * 0x11) << 16)
                | ((g as u32 * 0x11) << 8)
                | (b as u32 * 0x11);
            Some((0x1c, color)) // TYPE_INT_COLOR_ARGB8
        }
        // #RRGGBB
        6 => {
            let v = u32::from_str_radix(hex, 16).ok()?;
            Some((0x1d, 0xFF000000 | v)) // TYPE_INT_COLOR_RGB8
        }
        // #AARRGGBB
        8 => {
            let v = u32::from_str_radix(hex, 16).ok()?;
            Some((0x1c, v)) // TYPE_INT_COLOR_ARGB8
        }
        _ => None,
    }
}

pub fn parse_dimension(s: &str) -> Option<u32> {
    let (num_str, unit) = if let Some(n) = s.strip_suffix("dp") {
        (n, 1u32) // COMPLEX_UNIT_DIP
    } else if let Some(n) = s.strip_suffix("dip") {
        (n, 1u32)
    } else if let Some(n) = s.strip_suffix("sp") {
        (n, 2u32) // COMPLEX_UNIT_SP
    } else if let Some(n) = s.strip_suffix("pt") {
        (n, 3u32) // COMPLEX_UNIT_PT
    } else if let Some(n) = s.strip_suffix("in") {
        (n, 4u32) // COMPLEX_UNIT_IN
    } else if let Some(n) = s.strip_suffix("mm") {
        (n, 5u32) // COMPLEX_UNIT_MM
    } else if let Some(n) = s.strip_suffix("px") {
        (n, 0u32) // COMPLEX_UNIT_PX
    } else {
        return None;
    };

    let value: f32 = num_str.parse().ok()?;

    // Android complex dimension encoding:
    // bits 0-3: unit
    // bits 4-5: radix (0 = 23.0, 1 = 16.7, 2 = 8.15, 3 = 0.23)
    // bits 8-31: mantissa
    //
    // For integer values, use radix 0 (23.0 fixed point = integer << 8)
    // For fractional values, use radix 1 (16.7 fixed point)

    let int_val = value as i32;
    if (value - int_val as f32).abs() < f32::EPSILON && int_val.abs() < 0x800000 {
        // Integer dimension: radix 0, mantissa is the value
        let mantissa = (int_val as u32) & 0xFFFFFF;
        Some((mantissa << 8) | unit)
    } else {
        // Fractional: use radix 1 (16.7 fixed point)
        let scaled = (value * 128.0) as i32;
        let mantissa = (scaled as u32) & 0xFFFFFF;
        Some((mantissa << 8) | (1 << 4) | unit)
    }
}

/// Map common android:* attribute names to their framework resource IDs.
pub fn android_attr_res_id(name: &str) -> Option<u32> {
    Some(match name {
        "theme" => 0x0101_0000,
        "label" => 0x0101_0001,
        "icon" => 0x0101_0002,
        "name" => 0x0101_0003,
        "permission" => 0x0101_0006,
        "protectionLevel" => 0x0101_0009,
        "taskAffinity" => 0x0101_000d,
        "enabled" => 0x0101_000e,
        "exported" => 0x0101_000f,
        "process" => 0x0101_0011,
        "text" => 0x0101_0014,
        "textColor" => 0x0101_0098,
        "textSize" => 0x0101_0095,
        "textStyle" => 0x0101_0096,
        "typeface" => 0x0101_0097,
        "color" => 0x0101_0099,
        "id" => 0x0101_00d0,
        "tag" => 0x0101_00d1,
        "fitsSystemWindows" => 0x0101_00d2,
        "focusable" => 0x0101_00da,
        "visibility" => 0x0101_00dc,
        "scrollbarStyle" => 0x0101_00e0,
        "background" => 0x0101_00d4,
        "padding" => 0x0101_00d5,
        "paddingLeft" => 0x0101_00d6,
        "paddingTop" => 0x0101_00d7,
        "paddingRight" => 0x0101_00d8,
        "paddingBottom" => 0x0101_00d9,
        "clickable" => 0x0101_00e5,
        "longClickable" => 0x0101_00e6,
        "entries" => 0x0101_00b2,
        "layout" => 0x0101_00f2,
        "layout_width" => 0x0101_00f4,
        "layout_height" => 0x0101_00f5,
        "layout_weight" => 0x0101_00f6,
        "layout_gravity" => 0x0101_00f7,
        "layout_margin" => 0x0101_00f8,
        "layout_marginLeft" => 0x0101_00f9,
        "layout_marginTop" => 0x0101_00fa,
        "layout_marginRight" => 0x0101_00fb,
        "layout_marginBottom" => 0x0101_00fc,
        "gravity" => 0x0101_00af,
        "src" => 0x0101_0119,
        "tint" => 0x0101_0121,
        "orientation" => 0x0101_00c4,
        "scaleType" => 0x0101_011f,
        "contentDescription" => 0x0101_0273,
        "width" => 0x0101_0159,
        "height" => 0x0101_015a,
        "minWidth" => 0x0101_015e,
        "minHeight" => 0x0101_015f,
        "maxWidth" => 0x0101_015c,
        "configChanges" => 0x0101_001f,
        "screenOrientation" => 0x0101_001e,
        "launchMode" => 0x0101_001d,
        "value" => 0x0101_0024,
        "mimeType" => 0x0101_0026,
        "scheme" => 0x0101_0027,
        "host" => 0x0101_0028,
        "port" => 0x0101_0029,
        "path" => 0x0101_002a,
        "pathPrefix" => 0x0101_002b,
        "pathPattern" => 0x0101_002c,
        "authorities" => 0x0101_002d,
        "targetActivity" => 0x0101_0202,
        "versionCode" => 0x0101_021b,
        "versionName" => 0x0101_021c,
        "minSdkVersion" => 0x0101_020c,
        "targetSdkVersion" => 0x0101_0270,
        "maxSdkVersion" => 0x0101_0271,
        "windowSoftInputMode" => 0x0101_022b,
        "hardwareAccelerated" => 0x0101_0347,
        "allowBackup" => 0x0101_0280,
        "supportsRtl" => 0x0101_0383,
        "extractNativeLibs" => 0x0101_048c,
        "usesCleartextTraffic" => 0x0101_04ec,
        "roundIcon" => 0x0101_048f,
        "appComponentFactory" => 0x0101_057a,
        "networkSecurityConfig" => 0x0101_04f0,
        "debuggable" => 0x0101_0277,
        "viewportWidth" => 0x0101_0402,
        "viewportHeight" => 0x0101_0403,
        "pathData" => 0x0101_0405,
        "fillColor" => 0x0101_045e,
        "fillAlpha" => 0x0101_0480,
        "strokeColor" => 0x0101_0461,
        "strokeWidth" => 0x0101_0462,
        "strokeAlpha" => 0x0101_0481,
        "strokeLineCap" => 0x0101_0463,
        "strokeLineJoin" => 0x0101_0464,
        "strokeMiterLimit" => 0x0101_0465,
        "autoMirrored" => 0x0101_03ea,
        "alpha" => 0x0101_031f,
        "rotation" => 0x0101_0398,
        "scaleX" => 0x0101_0399,
        "scaleY" => 0x0101_039a,
        "pivotX" => 0x0101_0393,
        "pivotY" => 0x0101_0394,
        "translateX" => 0x0101_0466,
        "translateY" => 0x0101_0467,
        "key" => 0x0101_01e8,
        "title" => 0x0101_01e1,
        "summary" => 0x0101_01e3,
        "defaultValue" => 0x0101_01ed,
        "dependency" => 0x0101_01ec,
        "persistent" => 0x0101_01e5,
        "entryValues" => 0x0101_01f8,
        "dialogTitle" => 0x0101_01e6,
        "selectable" => 0x0101_03e8,
        "order" => 0x0101_01e2,
        "fragment" => 0x0101_0410,
        "splitName" => 0x0101_048a,
        "paddingStart" => 0x0101_03b3,
        "paddingEnd" => 0x0101_03b4,
        "layout_marginStart" => 0x0101_03b5,
        "layout_marginEnd" => 0x0101_03b6,
        "textAlignment" => 0x0101_038e,
        "importantForAccessibility" => 0x0101_03aa,
        "clipToPadding" => 0x0101_012e,
        "clipChildren" => 0x0101_012d,
        "foreground" => 0x0101_0109,
        "foregroundGravity" => 0x0101_010a,
        "elevation" => 0x0101_03f7,
        "translationZ" => 0x0101_03f8,
        "outlineProvider" => 0x0101_0413,
        "maxLines" => 0x0101_0062,
        "singleLine" => 0x0101_0063,
        "ellipsize" => 0x0101_00ab,
        "inputType" => 0x0101_01a2,
        "hint" => 0x0101_0150,
        "lines" => 0x0101_005d,
        "maxLength" => 0x0101_005e,
        "imeOptions" => 0x0101_022f,
        "drawableLeft" => 0x0101_016d,
        "drawableTop" => 0x0101_016e,
        "drawableRight" => 0x0101_016f,
        "drawableBottom" => 0x0101_0170,
        "drawablePadding" => 0x0101_0171,
        _ => return None,
    })
}

/// Returns true if the data looks like compiled AXML (starts with the magic bytes).
pub fn is_compiled_axml(data: &[u8]) -> bool {
    data.len() >= 8 && data[0] == 0x03 && data[1] == 0x00 && data[2] == 0x08 && data[3] == 0x00
}
