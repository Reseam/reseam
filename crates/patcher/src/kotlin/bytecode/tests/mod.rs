// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_apk::reseam_dex::{AccessFlags, DexFile, DexHeader, DexVersion};

use super::{
    annotations, assembly, class_ops, fields, lookup, methods, mutation, positions, registers,
    search,
};
use crate::context::PatchContext;
use crate::kotlin::handles::{ContextGuard, RunGuard, RunningBundle};
use crate::kotlin::types::{Instruction, NewMethod, SimpleInsn};

mod bridge;
mod handles;
mod lowering;
mod members;
mod operands;

fn fixture_method(name: String, proto: String, registers_size: u16, ins_size: u16) -> NewMethod {
    NewMethod {
        name,
        proto,
        registers_size,
        ins_size,
        access_flags: (AccessFlags::PUBLIC | AccessFlags::STATIC).bits(),
        outs_size: 0,
        instructions: vec![Instruction::Simple(SimpleInsn { opcode: 0x0e })],
        tries: Vec::new(),
        catch_handlers: Vec::new(),
    }
}

fn fixture_class(dex: u32, descriptor: &str) -> u32 {
    crate::kotlin::handles::with_ctx(|ctx| {
        ctx.dex_file_mut(dex as usize)
            .unwrap()
            .create_class(descriptor, AccessFlags::PUBLIC, Some("Ljava/lang/Object;"))
            .unwrap();
    });
    lookup::find_class(descriptor.into()).unwrap()
}

struct ExampleHandles {
    class: u32,
    method: u32,
    document: u32,
    root: u32,
}

fn example_handles(name: &str) -> ExampleHandles {
    let class = fixture_class(0, &format!("L{name};"));
    let method = methods::add_method(class, fixture_method("run".into(), "()V".into(), 0, 0));
    let document = crate::kotlin::xml::xml_compile(format!("<{name}/>")).unwrap();
    let root = crate::kotlin::xml::xml_root(document);
    ExampleHandles {
        class,
        method,
        document,
        root,
    }
}

fn previous_run_handles() -> ExampleHandles {
    let (directory, mut apk) = crate::test_support::apk();
    apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V035)));
    let mut context = PatchContext::new(&mut apk);
    let _run = RunGuard::enter().unwrap();
    let guard =
        ContextGuard::enter(&mut context, RunningBundle::at(directory.path().to_owned())).unwrap();
    let handles = example_handles("Old");
    guard.finish().unwrap();
    handles
}
