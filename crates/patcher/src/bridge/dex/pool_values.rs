// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_dex::{self, DexFile};

use super::convert::{invalid, resolve_field_ref, resolve_method_ref};
use super::encoded_values::{export_value, import_value};
use super::pool_copies::{PoolKind, intern_copy, pool_origin};
use super::types::{CallSiteRef, FieldHandleRef, HandleRef, MethodHandleRef};

pub(in crate::bridge) fn export_handle(
    dex: &DexFile,
    dex_index: Option<usize>,
    index: reseam_dex::MethodHandleIdx,
) -> reseam_dex::Result<HandleRef> {
    use reseam_dex::MethodHandleMember;
    let handle = dex.method_handle(index)?;
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

pub(in crate::bridge) fn import_handle(
    dex: &mut DexFile,
    dex_index: Option<usize>,
    handle: &HandleRef,
) -> reseam_dex::Result<reseam_dex::MethodHandleIdx> {
    use reseam_dex::{MethodHandle, MethodHandleMember, MethodHandleType};
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
    intern_copy(
        dex,
        origin,
        dex_index,
        PoolKind::Handle,
        |dex, candidates| {
            let candidates: Option<Vec<_>> =
                candidates.map(|c| c.into_iter().map(reseam_dex::MethodHandleIdx).collect());
            Ok(dex.intern_method_handle(handle, candidates.as_deref())?.0)
        },
    )
    .map(reseam_dex::MethodHandleIdx)
}

pub(in crate::bridge) fn export_call_site(
    dex: &DexFile,
    dex_index: Option<usize>,
    index: reseam_dex::CallSiteIdx,
) -> reseam_dex::Result<CallSiteRef> {
    let site = dex.call_site(index)?;
    Ok(CallSiteRef {
        origin: pool_origin(dex_index, index.0)?,
        bootstrap: export_handle(dex, dex_index, site.bootstrap_method)?,
        name: dex.strings().get(site.method_name).into_owned(),
        proto: dex.proto_descriptor(&dex.proto(site.method_type)),
        arguments: site
            .extra_arguments
            .iter()
            .map(|value| export_value(dex, dex_index, value))
            .collect::<reseam_dex::Result<_>>()?,
    })
}

pub(in crate::bridge) fn import_call_site(
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
    intern_copy(
        dex,
        origin,
        dex_index,
        PoolKind::CallSite,
        |dex, candidates| {
            let candidates: Option<Vec<_>> =
                candidates.map(|c| c.into_iter().map(reseam_dex::CallSiteIdx).collect());
            Ok(dex.intern_call_site(site, candidates.as_deref())?.0)
        },
    )
    .map(reseam_dex::CallSiteIdx)
}
