// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt::Write;

use crate::error::Result;
use crate::file::DexFile;
use crate::model::encoded_value::EncodedValue;
use crate::model::method_handle::{MethodHandleMember, MethodHandleType};
use crate::model::{
    CallSiteIdx, FieldIdx, MethodHandleIdx, MethodIdx, ProtoIdx, StringIdx, TypeIdx,
};

/// Writes pool references and encoded values the way baksmali does.
#[derive(Clone, Copy)]
pub struct SmaliNames<'a> {
    dex: &'a DexFile,
}

impl<'a> SmaliNames<'a> {
    pub fn new(dex: &'a DexFile) -> Self {
        Self { dex }
    }

    pub(super) fn string(self, idx: StringIdx) -> String {
        quote(&self.dex.strings().units(idx))
    }

    pub(super) fn type_(self, idx: TypeIdx) -> String {
        self.dex.type_descriptor(idx).into_owned()
    }

    pub(super) fn field(self, idx: FieldIdx) -> String {
        self.dex.field_descriptor(idx)
    }

    pub(super) fn method(self, idx: MethodIdx) -> String {
        self.dex.method_descriptor(idx)
    }

    pub(super) fn proto(self, idx: ProtoIdx) -> String {
        self.dex.proto_descriptor(&self.dex.proto(idx))
    }

    pub(super) fn method_handle(self, idx: MethodHandleIdx) -> Result<String> {
        let handle = self.dex.method_handle(idx)?;
        let member = match handle.member {
            MethodHandleMember::Field(field) => self.field(field),
            MethodHandleMember::Method(method) => self.method(method),
        };
        Ok(format!("{}@{member}", handle_type(handle.handle_type)))
    }

    /// `call_site_N("name", (args)R, extra...)@bootstrap`.
    pub(super) fn call_site(self, idx: CallSiteIdx) -> Result<String> {
        let site = self.dex.call_site(idx)?;
        let mut text = format!(
            "call_site_{}({}, {}",
            idx.0,
            self.string(site.method_name),
            self.proto(site.method_type),
        );
        for argument in &site.extra_arguments {
            write!(text, ", {}", self.value(argument)?).expect("writing to a String");
        }
        write!(text, ")@{}", self.method_handle(site.bootstrap_method)?)
            .expect("writing to a String");
        Ok(text)
    }

    pub fn value(self, value: &EncodedValue) -> Result<String> {
        Ok(match value {
            EncodedValue::Byte(value) => format!("{}t", hex(i64::from(*value))),
            EncodedValue::Short(value) => format!("{}s", hex(i64::from(*value))),
            EncodedValue::Char(value) => char_literal(*value),
            EncodedValue::Int(value) => hex(i64::from(*value)),
            EncodedValue::Long(value) => format!("{}L", hex(*value)),
            EncodedValue::Float(value) => format!("{}f", float(format!("{value:?}"))),
            EncodedValue::Double(value) => float(format!("{value:?}")),
            EncodedValue::MethodType(proto) => self.proto(*proto),
            EncodedValue::MethodHandle(handle) => self.method_handle(*handle)?,
            EncodedValue::String(string) => self.string(*string),
            EncodedValue::Type(type_) => self.type_(*type_),
            EncodedValue::Field(field) => self.field(*field),
            EncodedValue::Method(method) => self.method(*method),
            EncodedValue::Enum(field) => format!(".enum {}", self.field(*field)),
            EncodedValue::Array(values) => {
                let values = values
                    .iter()
                    .map(|value| self.value(value))
                    .collect::<Result<Vec<_>>>()?;
                format!("{{{}}}", values.join(", "))
            }
            EncodedValue::Annotation(annotation) => {
                let elements = annotation
                    .elements
                    .iter()
                    .map(|element| (element.name, &element.value));
                format!(
                    ".subannotation {}",
                    self.annotation(annotation.type_, elements)?
                )
            }
            EncodedValue::Null => "null".to_owned(),
            EncodedValue::Boolean(value) => value.to_string(),
        })
    }

    /// `Lpkg/Type; name = value, ...`.
    pub(super) fn annotation<'v>(
        self,
        type_: TypeIdx,
        elements: impl IntoIterator<Item = (StringIdx, &'v EncodedValue)>,
    ) -> Result<String> {
        let mut text = self.type_(type_);
        for (name, value) in elements {
            write!(
                text,
                " {} = {}",
                self.dex.strings().get(name),
                self.value(value)?
            )
            .expect("writing to a String");
        }
        Ok(text)
    }
}

fn handle_type(kind: MethodHandleType) -> &'static str {
    match kind {
        MethodHandleType::StaticPut => "static-put",
        MethodHandleType::StaticGet => "static-get",
        MethodHandleType::InstancePut => "instance-put",
        MethodHandleType::InstanceGet => "instance-get",
        MethodHandleType::InvokeStatic => "invoke-static",
        MethodHandleType::InvokeInstance => "invoke-instance",
        MethodHandleType::InvokeConstructor => "invoke-constructor",
        MethodHandleType::InvokeDirect => "invoke-direct",
        MethodHandleType::InvokeInterface => "invoke-interface",
    }
}

/// Signed hexadecimal as smali writes integer literals: `0x1f`, `-0x1`.
pub(super) fn hex(value: i64) -> String {
    if value < 0 {
        format!("-0x{:x}", value.unsigned_abs())
    } else {
        format!("0x{value:x}")
    }
}

/// A literal that may be wide: baksmali marks it `L` only when an `int` cannot hold it.
pub(super) fn long(value: i64) -> String {
    if i32::try_from(value).is_ok() {
        hex(value)
    } else {
        format!("{}L", hex(value))
    }
}

/// A float's shortest round-trip text as smali writes it: Rust's `inf` is
/// smali's `Infinity`.
fn float(text: String) -> String {
    match text.as_str() {
        "inf" => "Infinity".to_owned(),
        "-inf" => "-Infinity".to_owned(),
        _ => text,
    }
}

/// A string literal with smali's escapes, from the string's UTF-16 units so
/// unpaired surrogates print as they are in the file.
fn quote(units: &[u16]) -> String {
    let mut quoted = String::with_capacity(units.len() + 2);
    quoted.push('"');
    for &unit in units {
        escape(&mut quoted, unit);
    }
    quoted.push('"');
    quoted
}

fn char_literal(unit: u16) -> String {
    let mut quoted = String::from("'");
    escape(&mut quoted, unit);
    quoted.push('\'');
    quoted
}

/// Escapes one UTF-16 unit as baksmali does in string and char literals:
/// both quotes, the backslash, and every unit outside printable ASCII.
fn escape(out: &mut String, unit: u16) {
    match u8::try_from(unit).map(char::from) {
        Ok(c @ ('\\' | '"' | '\'')) => {
            out.push('\\');
            out.push(c);
        }
        Ok('\n') => out.push_str("\\n"),
        Ok('\r') => out.push_str("\\r"),
        Ok('\t') => out.push_str("\\t"),
        Ok(c @ ' '..='~') => out.push(c),
        _ => write!(out, "\\u{unit:04x}").expect("writing to a String"),
    }
}
