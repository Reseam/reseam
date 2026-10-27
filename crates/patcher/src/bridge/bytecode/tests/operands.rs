// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::*;

#[test]
fn pool_operands_survive_copying_between_dex_files() {
    use crate::bridge::dex::types::{EncodedVal, HandleRef};
    let (directory, mut apk) = crate::test_support::apk();
    apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V039)));
    let mut other = DexFile::new(DexHeader::new(DexVersion::V039));
    other
        .intern_method("LDifferent;", "unrelated", "(Z)I")
        .unwrap();
    apk.add_dex(other);
    let mut context = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
    let _run = RunGuard::enter().unwrap();
    let guard =
        ContextGuard::enter(&mut context, RunningBundle::at(directory.path().to_owned())).unwrap();
    let inputs = fixtures::pool_operands();
    let methods: Vec<_> = [(0, "LSource;"), (1, "LDestination;")]
        .into_iter()
        .map(|(dex, name)| {
            let class = fixture_class(dex, name);
            methods::add_method(
                class,
                NewMethod {
                    outs_size: 4,
                    instructions: if dex == 0 {
                        inputs.clone()
                    } else {
                        vec![Instruction::Simple(SimpleInsn { opcode: 0x0e })]
                    },
                    ..fixture_method("run".into(), "()V".into(), 4, 0)
                },
            )
        })
        .collect();
    guard.finish().unwrap();
    let source = crate::bridge::handles::method_location(methods[0]).unwrap();
    fixtures::duplicate_call_site(&mut context, source);
    let guard =
        ContextGuard::enter(&mut context, RunningBundle::at(directory.path().to_owned())).unwrap();
    mutation::replace_body(methods[1], 4, 4, search::get_instructions(methods[0])).unwrap();
    for &method in &methods {
        let instructions = search::get_instructions(method);
        let Instruction::RegProto(proto) = &instructions[0] else {
            panic!("method type operand was lost")
        };
        assert_eq!(proto.proto, "(IJ)Ljava/lang/String;");
        let Instruction::RegHandle(handle) = &instructions[1] else {
            panic!("method handle operand was lost")
        };
        let HandleRef::Method(handle) = &handle.handle else {
            panic!("handle member changed")
        };
        assert_eq!(handle.method.defining_class, "LBootstrap;");
        let Instruction::Polymorphic(call) = &instructions[2] else {
            panic!("polymorphic prototype was lost")
        };
        assert_eq!(call.proto, "(IJ)Ljava/lang/String;");
        assert_eq!(call.method.proto, "([Ljava/lang/Object;)Ljava/lang/Object;");
        let Instruction::Custom(call) = &instructions[3] else {
            panic!("call site was lost")
        };
        assert_eq!(call.call_site.name, "dynamic");
        let EncodedVal::ArrayVal(arguments) = &call.call_site.arguments[1] else {
            panic!("bootstrap array was lost")
        };
        let EncodedVal::FloatVal(nan) = arguments[1] else {
            panic!("bootstrap scalar kind changed")
        };
        assert_eq!(nan.to_bits(), 0x7fc0_0001);
        let EncodedVal::DoubleVal(zero) = arguments[2] else {
            panic!("bootstrap scalar kind changed")
        };
        assert_eq!(zero.to_bits(), (-0.0_f64).to_bits());
    }
    mutation::replace_body(methods[0], 4, 4, search::get_instructions(methods[0])).unwrap();
    guard.finish().unwrap();
    for dex in [0, 1] {
        assert_eq!(context.dex_file(dex).unwrap().call_sites().len(), 2);
    }
}

#[test]
fn authored_raw_pool_references_survive_insertion() {
    enum Reference {
        String,
        Type,
    }
    let (directory, mut apk) = crate::test_support::apk();
    apk.add_dex(DexFile::new(DexHeader::new(DexVersion::V039)));
    let mut context = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
    let _run = RunGuard::enter().unwrap();
    let guard =
        ContextGuard::enter(&mut context, RunningBundle::at(directory.path().to_owned())).unwrap();
    let handles = example_handles("RawReferences");
    registers::grow_local_registers(handles.method, 1, vec![]).unwrap();
    let intern = |pool: fn(&mut DexFile) -> u32| {
        crate::bridge::phase::with_ctx(|ctx| pool(ctx.dex_file_mut(0).unwrap()))
    };
    let references = [
        (
            Reference::String,
            (0x1a, intern(|dex| dex.intern_string("held string").0)),
        ),
        (
            Reference::Type,
            (0x1c, intern(|dex| dex.intern_type("LHeldType;").unwrap().0)),
        ),
    ];
    guard.finish().unwrap();
    for (reference, (opcode, index)) in references {
        let index = u16::try_from(index).unwrap().to_le_bytes();
        let raw = Instruction::Raw(vec![opcode, 0, index[0], index[1]]);
        let guard =
            ContextGuard::enter(&mut context, RunningBundle::at(directory.path().to_owned()))
                .unwrap();
        mutation::insert_instructions(handles.method, 0, vec![raw]).unwrap();
        match (reference, &search::get_instructions(handles.method)[0]) {
            (Reference::String, Instruction::RegString(load)) => {
                assert_eq!(load.value, "held string");
            }
            (Reference::Type, Instruction::RegType(load)) => {
                assert_eq!(load.type_descriptor, "LHeldType;");
            }
            _ => panic!("the raw reference changed its operand shape"),
        }
        guard.finish().unwrap();
    }
}
