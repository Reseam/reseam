// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Code that runs once at process start. Patches add it to one static hook
//! the engine owns; after every patch has run, the hook is called from
//! `onCreate()` of the `Application` the final manifest names. A patch that
//! swaps that class therefore takes every hook with it, whatever order the
//! patches ran in.

use reseam_apk::reseam_dex::{
    AccessFlags, ClassDef, CodeItem, DexFile, EncodedMethod, Instruction, RegList,
};

use super::{ClassLocation, MethodLocation, PatchContext};
use crate::error::{PatcherError, Result};

const HOOK_CLASS: &str = "Lapp/reseam/AppEntry;";
const HOOK_NAME: &str = "onCreate";
const HOOK_PROTO: &str = "(Landroid/app/Application;)V";
const APPLICATION: &str = "Landroid/app/Application;";
const OBJECT: &str = "Ljava/lang/Object;";

impl PatchContext<'_> {
    /// The static hook patches add start-up code to, created on first use.
    /// Fails when the manifest names no `Application`, since nothing would
    /// call it.
    pub fn app_entry_hook(&mut self) -> Result<MethodLocation> {
        self.application_class()?;
        if let Some(class) = self.find_class(HOOK_CLASS) {
            let dex = self.class_dex(class)?;
            let (method_idx, is_virtual) =
                declared_method(dex, class.class_idx, HOOK_NAME, HOOK_PROTO).ok_or_else(|| {
                    app_entry_error(format!("{HOOK_CLASS} has no {HOOK_NAME}{HOOK_PROTO}"))
                })?;
            return Ok(MethodLocation {
                dex_idx: class.dex_idx,
                class_idx: class.class_idx,
                method_idx,
                is_virtual,
            });
        }
        let dex = self
            .dex_file_mut(0)
            .ok_or_else(|| app_entry_error("the app has no DEX"))?;
        let class_idx = dex.create_class(
            HOOK_CLASS,
            AccessFlags::PUBLIC | AccessFlags::FINAL,
            Some(OBJECT),
        )?;
        let method = dex.intern_method(HOOK_CLASS, HOOK_NAME, HOOK_PROTO)?;
        let class = dex.class_mut(class_idx)?;
        class.add_direct_method(EncodedMethod {
            method,
            access_flags: AccessFlags::PUBLIC | AccessFlags::STATIC,
            code: Some(code(1, 1, 0, vec![Instruction::ReturnVoid])),
        });
        Ok(MethodLocation {
            dex_idx: 0,
            class_idx,
            method_idx: 0,
            is_virtual: false,
        })
    }

    /// Calls the hook first in the manifest `Application`'s `onCreate()`,
    /// adding that method when the class inherits it. Does nothing when no
    /// patch used the hook.
    pub(crate) fn bind_app_entry(&mut self) -> Result<()> {
        if self.find_class(HOOK_CLASS).is_none() {
            return Ok(());
        }
        let name = self.application_class()?;
        let descriptor = format!("L{};", name.replace('.', "/"));
        let application = self.find_class(&descriptor).ok_or_else(|| {
            app_entry_error(format!("Application class {descriptor} is not in the app"))
        })?;
        let dex = self.class_dex(application)?;
        match declared_method(dex, application.class_idx, "onCreate", "()V") {
            Some((method_idx, is_virtual)) => self.call_hook_on_entry(MethodLocation {
                dex_idx: application.dex_idx,
                class_idx: application.class_idx,
                method_idx,
                is_virtual,
            }),
            None => {
                self.unseal_inherited_on_create(application, &descriptor)?;
                self.add_on_create(application)
            }
        }
    }

    /// The fully qualified `<application android:name>` of the base manifest.
    fn application_class(&self) -> Result<String> {
        let manifest = self.apk().base().manifest();
        let name = manifest.application_name().ok_or_else(|| {
            app_entry_error("the manifest names no <application android:name>, so there is no app entry point to hook")
        })?;
        Ok(match manifest.package_name() {
            Some(package) if name.starts_with('.') => format!("{package}{name}"),
            Some(package) if !name.contains('.') => format!("{package}.{name}"),
            _ => name.into_owned(),
        })
    }

    fn class_dex(&mut self, class: ClassLocation) -> Result<&mut DexFile> {
        self.class_dex_mut(class.dex_idx, class.class_idx)
            .ok_or_else(|| {
                app_entry_error(format!(
                    "cannot load class {} of DEX {}",
                    class.class_idx, class.dex_idx
                ))
            })
    }

    fn call_hook_on_entry(&mut self, on_create: MethodLocation) -> Result<()> {
        let dex = self.class_dex(ClassLocation {
            dex_idx: on_create.dex_idx,
            class_idx: on_create.class_idx,
        })?;
        let hook = dex.intern_method(HOOK_CLASS, HOOK_NAME, HOOK_PROTO)?;
        let code = super::code_mut(dex, on_create)
            .ok_or_else(|| app_entry_error("the Application's onCreate() has no code"))?;
        code.outs_size = code.outs_size.max(1);
        code.insert_instruction(
            0,
            Instruction::InvokeStaticRange {
                method: hook,
                first_reg: code.registers_size - code.ins_size,
                count: 1,
            },
        )?;
        Ok(())
    }

    fn add_on_create(&mut self, application: ClassLocation) -> Result<()> {
        let dex = self.class_dex(application)?;
        let header = dex.class_header(application.class_idx);
        let class = dex.type_descriptor(header.class_type).into_owned();
        let superclass = header.superclass.map_or_else(
            || APPLICATION.to_owned(),
            |ty| dex.type_descriptor(ty).into_owned(),
        );
        let hook = dex.intern_method(HOOK_CLASS, HOOK_NAME, HOOK_PROTO)?;
        let inherited = dex.intern_method(&superclass, "onCreate", "()V")?;
        let method = dex.intern_method(&class, "onCreate", "()V")?;
        dex.class_mut(application.class_idx)?
            .add_virtual_method(EncodedMethod {
                method,
                access_flags: AccessFlags::PUBLIC,
                code: Some(code(
                    1,
                    1,
                    1,
                    vec![
                        Instruction::InvokeStatic {
                            method: hook,
                            args: RegList::from(&[0][..]),
                        },
                        Instruction::InvokeSuper {
                            method: inherited,
                            args: RegList::from(&[0][..]),
                        },
                        Instruction::ReturnVoid,
                    ],
                )),
            });
        Ok(())
    }

    /// Clears `final` on the `onCreate()` an added override would collide
    /// with, so ART accepts the override.
    fn unseal_inherited_on_create(
        &mut self,
        application: ClassLocation,
        descriptor: &str,
    ) -> Result<()> {
        for ancestor in self.superclass_chain(application) {
            let dex = self.class_dex(ancestor)?;
            let Some(slot) = declared_method(dex, ancestor.class_idx, "onCreate", "()V") else {
                continue;
            };
            let method = method_at(dex.class_mut(ancestor.class_idx)?, slot);
            // A private declaration is not in the vtable, so it is not what an override collides with.
            if method.access_flags.contains(AccessFlags::PRIVATE) {
                continue;
            }
            if method.access_flags.contains(AccessFlags::STATIC) {
                return Err(app_entry_error(format!(
                    "the inherited onCreate()V is static, so {descriptor} cannot override it"
                )));
            }
            method.access_flags.remove(AccessFlags::FINAL);
            return Ok(());
        }
        Ok(())
    }
}

/// The `(index, is_virtual)` slot of a method a materialized class declares.
fn declared_method(
    dex: &DexFile,
    class_idx: usize,
    name: &str,
    proto: &str,
) -> Option<(usize, bool)> {
    let data = dex.resident_class(class_idx)?.class_data.as_ref()?;
    let matches = |method: &EncodedMethod| {
        let id = dex.method_id(method.method);
        dex.string(id.name) == name && dex.proto_descriptor(&dex.proto(id.proto)) == proto
    };
    data.direct_methods
        .iter()
        .position(matches)
        .map(|idx| (idx, false))
        .or_else(|| {
            data.virtual_methods
                .iter()
                .position(matches)
                .map(|idx| (idx, true))
        })
}

fn method_at(class: &mut ClassDef, (idx, is_virtual): (usize, bool)) -> &mut EncodedMethod {
    let data = class.class_data.as_mut().expect("slot found in class data");
    if is_virtual {
        &mut data.virtual_methods[idx]
    } else {
        &mut data.direct_methods[idx]
    }
}

fn code(
    registers_size: u16,
    ins_size: u16,
    outs_size: u16,
    instructions: Vec<Instruction>,
) -> CodeItem {
    CodeItem {
        registers_size,
        ins_size,
        outs_size,
        debug_info: None,
        instructions,
        tries: Vec::new(),
        catch_handlers: Vec::new(),
    }
}

fn app_entry_error(message: impl Into<String>) -> PatcherError {
    PatcherError::AppEntry(message.into())
}
