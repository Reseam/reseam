// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_dex::{self, DexFile};

use super::convert::{invalid, resolve_field_ref, resolve_method_ref};
use super::pool_values::{export_handle, import_handle};
use super::types::{AnnotationElement, AnnotationValue, EncodedVal};

pub(in crate::bridge) fn export_value(
    dex: &DexFile,
    dex_index: Option<usize>,
    value: &reseam_dex::EncodedValue,
) -> reseam_dex::Result<EncodedVal> {
    use reseam_dex::EncodedValue as V;
    Ok(match value {
        V::Null => EncodedVal::Null,
        V::Boolean(v) => EncodedVal::BoolVal(*v),
        V::Byte(v) => EncodedVal::ByteVal(*v),
        V::Short(v) => EncodedVal::ShortVal(*v),
        V::Char(v) => EncodedVal::CharVal(*v),
        V::Int(v) => EncodedVal::IntVal(*v),
        V::Long(v) => EncodedVal::LongVal(*v),
        V::Float(v) => EncodedVal::FloatVal(*v),
        V::Double(v) => EncodedVal::DoubleVal(*v),
        V::String(v) => EncodedVal::StringVal(dex.strings().get(*v).into_owned()),
        V::Type(v) => EncodedVal::TypeVal(dex.type_descriptor(*v).into_owned()),
        V::MethodType(v) => EncodedVal::ProtoVal(dex.proto_descriptor(&dex.proto(*v))),
        V::MethodHandle(v) => EncodedVal::HandleVal(export_handle(dex, dex_index, *v)?),
        V::Field(v) => EncodedVal::FieldVal(resolve_field_ref(dex, *v)),
        V::Method(v) => EncodedVal::MethodVal(resolve_method_ref(dex, *v)),
        V::Enum(v) => EncodedVal::EnumVal(resolve_field_ref(dex, *v)),
        V::Array(v) => EncodedVal::ArrayVal(
            v.iter()
                .map(|v| export_value(dex, dex_index, v))
                .collect::<reseam_dex::Result<_>>()?,
        ),
        V::Annotation(v) => EncodedVal::AnnotationVal(AnnotationValue {
            annotation_type: dex.type_descriptor(v.type_).into_owned(),
            elements: v
                .elements
                .iter()
                .map(|element| {
                    Ok(AnnotationElement {
                        name: dex.strings().get(element.name).into_owned(),
                        value: export_value(dex, dex_index, &element.value)?,
                    })
                })
                .collect::<reseam_dex::Result<_>>()?,
        }),
        _ => return Err(invalid("unsupported encoded value")),
    })
}

pub(in crate::bridge) fn import_value(
    dex: &mut DexFile,
    dex_index: Option<usize>,
    value: &EncodedVal,
) -> reseam_dex::Result<reseam_dex::EncodedValue> {
    use reseam_dex::EncodedValue as V;
    use reseam_dex::{EncodedAnnotation, EncodedAnnotationElement};
    Ok(match value {
        EncodedVal::Null => V::Null,
        EncodedVal::BoolVal(v) => V::Boolean(*v),
        EncodedVal::ByteVal(v) => V::Byte(*v),
        EncodedVal::ShortVal(v) => V::Short(*v),
        EncodedVal::CharVal(v) => V::Char(*v),
        EncodedVal::IntVal(v) => V::Int(*v),
        EncodedVal::LongVal(v) => V::Long(*v),
        EncodedVal::FloatVal(v) => V::Float(*v),
        EncodedVal::DoubleVal(v) => V::Double(*v),
        EncodedVal::StringVal(v) => V::String(dex.intern_string(v)),
        EncodedVal::TypeVal(v) => V::Type(dex.intern_type(v)?),
        EncodedVal::ProtoVal(v) => V::MethodType(dex.intern_proto(v)?),
        EncodedVal::HandleVal(v) => V::MethodHandle(import_handle(dex, dex_index, v)?),
        EncodedVal::FieldVal(v) => {
            V::Field(dex.intern_field(&v.defining_class, &v.name, &v.field_type)?)
        }
        EncodedVal::MethodVal(v) => {
            V::Method(dex.intern_method(&v.defining_class, &v.name, &v.proto)?)
        }
        EncodedVal::EnumVal(v) => {
            V::Enum(dex.intern_field(&v.defining_class, &v.name, &v.field_type)?)
        }
        EncodedVal::ArrayVal(v) => V::Array(
            v.iter()
                .map(|v| import_value(dex, dex_index, v))
                .collect::<reseam_dex::Result<_>>()?,
        ),
        EncodedVal::AnnotationVal(v) => V::Annotation(EncodedAnnotation {
            type_: dex.intern_type(&v.annotation_type)?,
            elements: v
                .elements
                .iter()
                .map(|element| {
                    Ok(EncodedAnnotationElement {
                        name: dex.intern_string(&element.name),
                        value: import_value(dex, dex_index, &element.value)?,
                    })
                })
                .collect::<reseam_dex::Result<_>>()?,
        }),
    })
}
