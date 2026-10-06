// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs::File;
use std::io::Write;

use reseam_apk::{ApkFile, axml};

pub(crate) fn apk() -> (tempfile::TempDir, ApkFile) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("base.apk");
    let mut archive = zip::ZipWriter::new(File::create(&path).unwrap());
    archive
        .start_file(
            "AndroidManifest.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    archive.write_all(&axml::compile_xml(
        r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example" android:versionName="2.0"><uses-sdk android:minSdkVersion="21"/><application><activity android:name=".Main"/><activity android:name=".Other"/></application></manifest>"#,
        None,
    ).unwrap()).unwrap();
    archive.finish().unwrap();
    let apk = ApkFile::open(path, ApkFile::patch_options()).unwrap();
    (dir, apk)
}

pub(crate) fn pool_operands() -> Vec<crate::kotlin::types::Instruction> {
    use crate::kotlin::types::{
        CallSiteRef, CustomInsn, EncodedVal, HandleRef, Instruction, MethodHandleRef, MethodRef,
        PolymorphicInsn, RegHandleInsn, RegProtoInsn, SimpleInsn,
    };
    let bootstrap = HandleRef::Method(MethodHandleRef {
        origin: None,
        kind: 4,
        method: MethodRef {
            defining_class: "LBootstrap;".into(),
            name: "bootstrap".into(),
            proto: "()Ljava/lang/invoke/CallSite;".into(),
        },
    });
    vec![
        Instruction::RegProto(RegProtoInsn {
            opcode: 0xff,
            reg_a: 0,
            proto: "(IJ)Ljava/lang/String;".into(),
        }),
        Instruction::RegHandle(RegHandleInsn {
            opcode: 0xfe,
            reg_a: 0,
            handle: bootstrap.clone(),
        }),
        Instruction::Polymorphic(PolymorphicInsn {
            opcode: 0xfa,
            method: MethodRef {
                defining_class: "Ljava/lang/invoke/MethodHandle;".into(),
                name: "invokeExact".into(),
                proto: "([Ljava/lang/Object;)Ljava/lang/Object;".into(),
            },
            proto: "(IJ)Ljava/lang/String;".into(),
            registers: vec![0, 1, 2, 3],
        }),
        Instruction::Custom(CustomInsn {
            opcode: 0xfc,
            call_site: CallSiteRef {
                origin: None,
                bootstrap,
                name: "dynamic".into(),
                proto: "()V".into(),
                arguments: vec![
                    EncodedVal::ProtoVal("(I)J".into()),
                    EncodedVal::ArrayVal(vec![
                        EncodedVal::StringVal("payload".into()),
                        EncodedVal::FloatVal(f32::from_bits(0x7fc0_0001)),
                        EncodedVal::DoubleVal(-0.0),
                    ]),
                ],
            },
            registers: Vec::new(),
        }),
        Instruction::Simple(SimpleInsn { opcode: 0x0e }),
    ]
}

pub(crate) fn duplicate_call_site(
    context: &mut crate::context::PatchContext<'_>,
    source: crate::context::MethodLocation,
) {
    let dex = context.dex_file_mut(0).unwrap();
    let site = dex.call_sites().get(0).unwrap().into_owned();
    let duplicate = dex.call_sites_mut().push(site);
    crate::context::code_mut(dex, source)
        .unwrap()
        .unwrap()
        .insert_instructions(
            4,
            &[reseam_apk::reseam_dex::Instruction::InvokeCustom {
                call_site: reseam_apk::reseam_dex::CallSiteIdx(duplicate as u32),
                args: reseam_apk::reseam_dex::RegList::default(),
            }],
        )
        .unwrap();
}

pub(crate) fn branch_marker() -> crate::kotlin::types::NewMethod {
    use crate::kotlin::types::{BranchInsn, Instruction, NewMethod, RegStringInsn, SimpleInsn};
    NewMethod {
        name: "branchMarker".into(),
        proto: "(I)V".into(),
        access_flags: 9,
        registers_size: 1,
        ins_size: 1,
        outs_size: 0,
        tries: Vec::new(),
        catch_handlers: Vec::new(),
        instructions: [
            Instruction::Branch(BranchInsn {
                opcode: 0x38,
                reg_a: 0,
                offset: 32764,
            }),
            Instruction::RegString(RegStringInsn {
                opcode: 0x1a,
                reg_a: 0,
                value: "held".into(),
            }),
        ]
        .into_iter()
        .chain(std::iter::repeat_n(
            Instruction::Simple(SimpleInsn { opcode: 0 }),
            32760,
        ))
        .chain([Instruction::Simple(SimpleInsn { opcode: 0x0e })])
        .collect(),
    }
}

pub(crate) fn invoke_frame() -> crate::kotlin::types::NewMethod {
    use crate::kotlin::types::{
        Instruction, InvokeInsn, MethodRef, NewMethod, RegLiteralInsn, SimpleInsn,
    };
    NewMethod {
        name: "invokeFrame".into(),
        proto: "(JI)V".into(),
        access_flags: 9,
        registers_size: 16,
        ins_size: 3,
        outs_size: 3,
        tries: Vec::new(),
        catch_handlers: Vec::new(),
        instructions: vec![
            Instruction::RegLiteral(RegLiteralInsn {
                opcode: 0x16,
                reg_a: 10,
                reg_b: 0,
                literal: 42,
            }),
            Instruction::Invoke(InvokeInsn {
                opcode: 0x71,
                registers: vec![10, 11, 15],
                method: MethodRef {
                    defining_class: "LObserver;".into(),
                    name: "observe".into(),
                    proto: "(JI)V".into(),
                },
            }),
            Instruction::Simple(SimpleInsn { opcode: 0x0e }),
        ],
    }
}
