// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::logged;
use crate::context::{ClassLocation, MethodLocation};
use crate::kotlin::handles::{
    alloc_method, forget_method, method_ref, with_class_mut, with_method_mut,
};
use crate::kotlin::link::{link_descriptors, link_instructions, proto_types};
use crate::kotlin::types::NewMethod;
use boltffi::export;
use reseam_apk::reseam_dex::MethodKind;
use reseam_apk::reseam_dex::{
    AccessFlags, CatchHandler as DexCatchHandler, ClassDef, DexFile, EncodedMethod,
    TryItem as DexTryItem, TypedCatch as DexTypedCatch,
};

#[export]
pub fn add_method(c: u32, method: NewMethod) -> u32 {
    link_instructions(&method.instructions);
    link_descriptors(
        proto_types(&method.proto).into_iter().chain(
            method
                .catch_handlers
                .iter()
                .flat_map(|handler| handler.typed_catches.iter())
                .map(|catch| catch.exception_type.as_str()),
        ),
    );
    with_class_mut(c, |dex, loc| {
        let class_desc = dex
            .type_descriptor(dex.class_header(loc.class_idx).class_type)
            .into_owned();
        let method_idx = logged(
            "intern method",
            dex.intern_method(&class_desc, &method.name, &method.proto),
        )?;
        let flags = AccessFlags::from_bits_retain(method.access_flags);
        let code = if flags.intersects(AccessFlags::NATIVE | AccessFlags::ABSTRACT) {
            if !method.instructions.is_empty()
                || !method.tries.is_empty()
                || !method.catch_handlers.is_empty()
            {
                return Err("native or abstract methods cannot have a body".into());
            }
            None
        } else {
            let incoming = logged(
                "method frame",
                crate::kotlin::invoke::incoming_words(&method.proto, flags),
            )?;
            if incoming != method.ins_size {
                return Err(format!(
                    "method declares {} incoming words, prototype requires {incoming}",
                    method.ins_size
                ));
            }
            let assembly = logged(
                "assemble method body",
                super::assembly::assemble(
                    method.instructions,
                    Vec::new(),
                    dex,
                    Some(loc.dex_idx),
                    |code, dex| {
                        code.set_register_frame(
                            method.registers_size,
                            method.ins_size,
                            method.outs_size,
                        )?;
                        let tries = method
                            .tries
                            .iter()
                            .map(|item| DexTryItem {
                                start_addr: item.start_addr,
                                insn_count: item.insn_count,
                                handler_idx: item.handler_idx as usize,
                            })
                            .collect();
                        let handlers = method
                            .catch_handlers
                            .iter()
                            .map(|handler| {
                                Ok(DexCatchHandler {
                                    typed_catches: handler
                                        .typed_catches
                                        .iter()
                                        .map(|catch| {
                                            Ok(DexTypedCatch {
                                                exception_type: dex
                                                    .intern_type(&catch.exception_type)?,
                                                addr: catch.addr,
                                            })
                                        })
                                        .collect::<reseam_apk::reseam_dex::Result<_>>()?,
                                    catch_all_addr: handler.catch_all_addr,
                                })
                            })
                            .collect::<reseam_apk::reseam_dex::Result<_>>()?;
                        code.set_exception_handlers(tries, handlers)
                    },
                ),
            )?;
            Some(assembly.code)
        };
        let kind = method_kind(flags);
        let encoded = EncodedMethod {
            method: method_idx,
            access_flags: flags,
            code,
        };
        Ok(push_method(
            logged("add method", dex.class_mut(loc.class_idx))?,
            loc,
            encoded,
            kind,
        ))
    })
    .unwrap_or(0)
}

fn method_kind(flags: AccessFlags) -> MethodKind {
    if flags.intersects(AccessFlags::STATIC | AccessFlags::CONSTRUCTOR | AccessFlags::PRIVATE) {
        MethodKind::Direct
    } else {
        MethodKind::Virtual
    }
}

fn push_method(
    class: &mut ClassDef,
    loc: ClassLocation,
    method: EncodedMethod,
    kind: MethodKind,
) -> Option<u32> {
    if kind == MethodKind::Virtual {
        class.add_virtual_method(method);
    } else {
        class.add_direct_method(method);
    }
    let data = class.class_data.as_ref()?;
    let list = if kind == MethodKind::Virtual {
        &data.virtual_methods
    } else {
        &data.direct_methods
    };
    Some(alloc_method(MethodLocation {
        dex_idx: loc.dex_idx,
        class_idx: loc.class_idx,
        method_idx: list.len() - 1,
        kind,
    }))
}

#[export]
pub fn remove_method(m: u32) {
    with_method_mut(m, |dex, loc| {
        let class = logged("remove method", dex.class_mut(loc.class_idx))?;
        let Some(data) = class.class_data.as_mut() else {
            return Ok(None);
        };
        let list = if loc.kind == MethodKind::Virtual {
            &mut data.virtual_methods
        } else {
            &mut data.direct_methods
        };
        let Some(method) = list.get(loc.method_idx).map(|entry| entry.method) else {
            return Ok(None);
        };
        list.remove(loc.method_idx);
        forget_method(loc);
        if let Some(metadata) = class.annotations.as_mut() {
            let annotations = logged("read annotations", metadata.resolve_mut())?;
            annotations.methods.retain(|(id, _)| *id != method);
            annotations.parameters.retain(|(id, _)| *id != method);
        }
        Ok(Some(()))
    });
}

#[export]
pub fn set_method_access_flags(
    m: u32,
    flags: u32,
) -> Result<Option<crate::kotlin::types::MethodEdit>, String> {
    let outcome = with_method_mut(m, |dex, loc| {
        let flags = AccessFlags::from_bits_retain(flags);
        let method = logged("access method", crate::context::method_mut(dex, loc))?
            .ok_or_else(|| format!("method handle {m} is missing"))?;
        let (id, previous_flags) = (method.method, method.access_flags);
        let mut code = method.code.take();
        let frame = prepare_flag_frame(dex, id, previous_flags, flags, &mut code);
        logged("restore method", crate::context::method_mut(dex, loc))?
            .expect("frame preparation leaves the method slot in place")
            .code = code;
        let mapping = frame?;
        let class = logged("change method flags", dex.class_mut(loc.class_idx))?;
        let data = class
            .class_data
            .as_mut()
            .expect("method was resolved in class data");
        let source = match loc.kind {
            MethodKind::Direct => &mut data.direct_methods,
            MethodKind::Virtual => &mut data.virtual_methods,
        };
        let method = source
            .get_mut(loc.method_idx)
            .expect("resolved method retains its slot");
        method.access_flags = flags;
        let kind = method_kind(flags);
        if kind != loc.kind {
            let method = source.remove(loc.method_idx);
            let destination = match kind {
                MethodKind::Direct => &mut data.direct_methods,
                MethodKind::Virtual => &mut data.virtual_methods,
            };
            let new = MethodLocation {
                kind,
                method_idx: destination.len(),
                ..loc
            };
            destination.push(method);
            crate::kotlin::handles::relocate_method(loc, new);
        }
        Ok(Some(mapping))
    });
    crate::kotlin::handles::check_call()?;
    outcome.ok_or_else(|| format!("method handle {m} could not be changed"))
}

fn prepare_flag_frame(
    dex: &DexFile,
    method: reseam_apk::reseam_dex::MethodIdx,
    previous: AccessFlags,
    flags: AccessFlags,
    code: &mut Option<reseam_apk::reseam_dex::CodeItem>,
) -> Result<Option<crate::kotlin::types::MethodEdit>, String> {
    if flags.intersects(AccessFlags::ABSTRACT | AccessFlags::NATIVE) {
        *code = None;
        return Ok(None);
    }
    let Some(code) = code.as_mut() else {
        return Ok(None);
    };
    let proto = dex.proto_descriptor(&dex.proto(dex.method_id(method).proto));
    let incoming = logged(
        "change method frame",
        crate::kotlin::invoke::incoming_words(&proto, flags),
    )?;
    let growth = incoming.saturating_sub(code.registers_size());
    let mapping = if growth == 0 {
        None
    } else {
        let types = super::registers::incoming_types(dex, method, previous);
        let mut mapping = super::mutation::edit_mapping(logged(
            "grow receiver frame",
            reseam_apk::reseam_dex::types::register_allocation::grow_registers(
                code,
                growth,
                &types,
                &[],
                dex,
            ),
        )?)?;
        mapping.register_shift = growth;
        Some(mapping)
    };
    logged(
        "change method frame",
        code.set_register_frame(code.registers_size(), incoming, code.outs_size()),
    )?;
    Ok(mapping)
}

#[export]
pub fn clone_method(m: u32, new_name: Option<String>) -> u32 {
    with_method_mut(m, |dex, loc| {
        let Some(method) = method_ref(dex, loc).cloned() else {
            return Ok(None);
        };
        let method_idx = match new_name {
            Some(name) => {
                let id = dex.method_id(method.method);
                let class_desc = dex.type_descriptor(id.class).into_owned();
                let proto = dex.proto_descriptor(&dex.proto(id.proto));
                logged(
                    "intern cloned method",
                    dex.intern_method(&class_desc, &name, &proto),
                )?
            }
            None => method.method,
        };
        let cloned = EncodedMethod {
            method: method_idx,
            ..method
        };
        let class_loc = ClassLocation {
            dex_idx: loc.dex_idx,
            class_idx: loc.class_idx,
        };
        Ok(push_method(
            logged("clone method", dex.class_mut(loc.class_idx))?,
            class_loc,
            cloned,
            loc.kind,
        ))
    })
    .unwrap_or(0)
}
