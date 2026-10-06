// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::types::PoolOrigin;
use super::types::{
    AnnotationElement, AnnotationValue, CallSiteRef, EncodedVal, FieldHandleRef, HandleRef,
    MethodHandleRef,
};
use reseam_apk::reseam_dex::{self, DexFile};

use super::convert::{invalid, resolve_field_ref, resolve_method_ref};

// A transient assembly pool has no application identity and never changes run caches.
fn pool_origin(dex_index: Option<usize>, index: u32) -> reseam_dex::Result<Option<PoolOrigin>> {
    dex_index
        .map(|dex_index| {
            Ok(PoolOrigin {
                dex_index: u32::try_from(dex_index)
                    .map_err(|_| invalid("DEX index exceeds 32 bits"))?,
                index,
            })
        })
        .transpose()
}

pub(super) fn export_handle(
    dex: &DexFile,
    dex_index: Option<usize>,
    index: reseam_dex::MethodHandleIdx,
) -> reseam_dex::Result<HandleRef> {
    use reseam_dex::types::method_handle::MethodHandleMember;
    let handle = dex.method_handles().get(index.0 as usize)?;
    Ok(match handle.member {
        MethodHandleMember::Field(field) => HandleRef::Field(FieldHandleRef {
            origin: pool_origin(dex_index, index.0)?,
            kind: handle.handle_type.to_u16(),
            field: resolve_field_ref(dex, field),
        }),
        MethodHandleMember::Method(method) => HandleRef::Method(MethodHandleRef {
            origin: pool_origin(dex_index, index.0)?,
            kind: handle.handle_type.to_u16(),
            method: resolve_method_ref(dex, method),
        }),
    })
}

pub(super) fn import_handle(
    dex: &mut DexFile,
    dex_index: Option<usize>,
    handle: &HandleRef,
) -> reseam_dex::Result<reseam_dex::MethodHandleIdx> {
    use reseam_dex::types::method_handle::{MethodHandle, MethodHandleMember, MethodHandleType};
    let origin = match handle {
        HandleRef::Field(value) => value.origin,
        HandleRef::Method(value) => value.origin,
    };
    let (kind, member) = match handle {
        HandleRef::Field(value) => (
            value.kind,
            MethodHandleMember::Field(dex.intern_field(
                &value.field.defining_class,
                &value.field.name,
                &value.field.field_type,
            )?),
        ),
        HandleRef::Method(value) => (
            value.kind,
            MethodHandleMember::Method(dex.intern_method(
                &value.method.defining_class,
                &value.method.name,
                &value.method.proto,
            )?),
        ),
    };
    let handle_type =
        MethodHandleType::from_u16(kind).ok_or_else(|| invalid("unknown method handle kind"))?;
    if handle_type.is_field() != matches!(member, MethodHandleMember::Field(_)) {
        return Err(invalid("method handle kind does not match its member"));
    }
    let handle = MethodHandle {
        handle_type,
        member,
    };
    Ok(reseam_dex::MethodHandleIdx(intern_pool(
        dex.method_handles_mut(),
        handle,
        origin,
        dex_index,
        super::handles::PoolKind::Handle,
        MethodHandle::eq,
    )?))
}

pub(super) fn export_call_site(
    dex: &DexFile,
    dex_index: Option<usize>,
    index: reseam_dex::CallSiteIdx,
) -> reseam_dex::Result<CallSiteRef> {
    let site = dex.call_sites().get(index.0 as usize)?;
    Ok(CallSiteRef {
        origin: pool_origin(dex_index, index.0)?,
        bootstrap: export_handle(dex, dex_index, site.bootstrap_method)?,
        name: dex.string(site.method_name).into_owned(),
        proto: dex.proto_descriptor(&dex.proto(site.method_type)),
        arguments: site
            .extra_arguments
            .iter()
            .map(|value| export_value(dex, dex_index, value))
            .collect::<reseam_dex::Result<_>>()?,
    })
}

pub(super) fn import_call_site(
    dex: &mut DexFile,
    dex_index: Option<usize>,
    site: &CallSiteRef,
) -> reseam_dex::Result<reseam_dex::CallSiteIdx> {
    let origin = site.origin;
    let site = reseam_dex::CallSiteItem {
        bootstrap_method: import_handle(dex, dex_index, &site.bootstrap)?,
        method_name: dex.intern_string(&site.name),
        method_type: dex.intern_proto(&site.proto)?,
        extra_arguments: site
            .arguments
            .iter()
            .map(|value| import_value(dex, dex_index, value))
            .collect::<reseam_dex::Result<_>>()?,
    };
    Ok(reseam_dex::CallSiteIdx(intern_pool(
        dex.call_sites_mut(),
        site,
        origin,
        dex_index,
        super::handles::PoolKind::CallSite,
        call_site_eq,
    )?))
}

fn intern_pool<T: reseam_dex::file::FileRecord>(
    table: &mut reseam_dex::file::FileTable<T>,
    value: T,
    origin: Option<PoolOrigin>,
    destination: Option<usize>,
    kind: super::handles::PoolKind,
    equal: impl Fn(&T, &T) -> bool,
) -> reseam_dex::Result<u32> {
    let destination = destination
        .map(u32::try_from)
        .transpose()
        .map_err(|_| invalid("DEX index exceeds 32 bits"))?;
    let origin = origin.filter(|_| destination.is_some());
    let existing = if let (Some(origin), Some(destination)) = (origin, destination) {
        let original = (origin.dex_index == destination).then_some(origin.index);
        let copied = super::handles::copied_pool(origin, destination, kind);
        original
            .into_iter()
            .chain(copied)
            .try_fold(None, |existing, index| {
                if existing.is_some() {
                    return Ok(existing);
                }
                table
                    .get(index as usize)
                    .map(|candidate| equal(&candidate, &value).then_some(index))
            })?
    } else {
        table
            .iter()
            .enumerate()
            .find_map(|(index, candidate)| match candidate {
                Ok(candidate) if equal(&candidate, &value) => {
                    Some(u32::try_from(index).map_err(|_| invalid("pool exceeds 32 bits")))
                }
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .transpose()?
    };
    let index = match existing {
        Some(index) => index,
        None => u32::try_from(table.push(value)).map_err(|_| invalid("pool exceeds 32 bits"))?,
    };
    if let Some(origin) = origin
        && let Some(destination) = destination
    {
        super::handles::remember_pool(origin, destination, kind, index);
    }
    Ok(index)
}

fn call_site_eq(left: &reseam_dex::CallSiteItem, right: &reseam_dex::CallSiteItem) -> bool {
    left.bootstrap_method == right.bootstrap_method
        && left.method_name == right.method_name
        && left.method_type == right.method_type
        && values_eq(&left.extra_arguments, &right.extra_arguments)
}

fn values_eq(left: &[reseam_dex::EncodedValue], right: &[reseam_dex::EncodedValue]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| value_eq(left, right))
}

fn value_eq(left: &reseam_dex::EncodedValue, right: &reseam_dex::EncodedValue) -> bool {
    use reseam_dex::EncodedValue as V;
    match (left, right) {
        (V::Float(left), V::Float(right)) => left.to_bits() == right.to_bits(),
        (V::Double(left), V::Double(right)) => left.to_bits() == right.to_bits(),
        (V::Array(left), V::Array(right)) => values_eq(left, right),
        (V::Annotation(left), V::Annotation(right)) => {
            left.type_ == right.type_
                && left.elements.len() == right.elements.len()
                && left
                    .elements
                    .iter()
                    .zip(&right.elements)
                    .all(|(left, right)| {
                        left.name == right.name && value_eq(&left.value, &right.value)
                    })
        }
        _ => left == right,
    }
}

pub(super) fn export_value(
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
        V::String(v) => EncodedVal::StringVal(dex.string(*v).into_owned()),
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
                        name: dex.string(element.name).into_owned(),
                        value: export_value(dex, dex_index, &element.value)?,
                    })
                })
                .collect::<reseam_dex::Result<_>>()?,
        }),
        _ => return Err(invalid("unsupported encoded value")),
    })
}

pub(super) fn import_value(
    dex: &mut DexFile,
    dex_index: Option<usize>,
    value: &EncodedVal,
) -> reseam_dex::Result<reseam_dex::EncodedValue> {
    use reseam_dex::EncodedValue as V;
    use reseam_dex::types::encoded_value::{EncodedAnnotation, EncodedAnnotationElement};
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
