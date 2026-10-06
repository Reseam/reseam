// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_dex::types::access_flags::AccessFlags;
use reseam_dex::types::annotation::{AnnotationItem, AnnotationVisibility, AnnotationsDirectory};
use reseam_dex::types::class::{EncodedField, EncodedMethod};
use reseam_dex::types::debug::{DebugBytecode, DebugInfo};
use reseam_dex::types::metadata::Metadata;
use reseam_dex::{CodeItem, DexFile, DexHeader, DexVersion, EncodedValue, Instruction};

pub fn dex() -> DexFile {
    let mut dex = DexFile::new(DexHeader::new(DexVersion::V039));
    let class = dex
        .create_class(
            "Lexample/Activity;",
            AccessFlags::PUBLIC,
            Some("Ljava/lang/Object;"),
        )
        .expect("valid fixture class");
    let string = dex.intern_string("screen opened");
    let annotation = dex
        .intern_type("Lexample/Trace;")
        .expect("valid fixture descriptor");
    let field = dex
        .intern_field("Lexample/Activity;", "counter", "I")
        .expect("valid fixture field");
    let direct = dex
        .intern_method("Lexample/Activity;", "report", "()V")
        .expect("valid fixture method");
    let virtual_method = dex
        .intern_method("Lexample/Activity;", "display", "()Ljava/lang/String;")
        .expect("valid fixture method");
    let class = dex.class_mut(class).expect("fixture class exists");
    class.add_static_field(EncodedField {
        field,
        access_flags: AccessFlags::PUBLIC | AccessFlags::STATIC,
    });
    class.static_values.insert(field, EncodedValue::Int(7));
    class.annotations = Some(Box::new(Metadata::new(AnnotationsDirectory {
        class: vec![AnnotationItem {
            visibility: AnnotationVisibility::Runtime,
            type_: annotation,
            elements: Vec::new(),
        }],
        ..AnnotationsDirectory::default()
    })));
    let mut code = CodeItem::new(
        2,
        0,
        0,
        vec![
            Instruction::ConstString { dest: 0, string },
            Instruction::ReturnVoid,
        ],
    )
    .expect("valid fixture frame");
    code.set_debug_info(Some(Metadata::new(DebugInfo {
        line_start: 42,
        parameter_names: Vec::new(),
        bytecodes: vec![
            DebugBytecode::SpecialAdvance {
                line_advance: 0,
                pc_advance: 2,
            },
            DebugBytecode::EndSequence,
        ],
    })));
    class.add_direct_method(EncodedMethod {
        method: direct,
        access_flags: AccessFlags::PUBLIC | AccessFlags::STATIC,
        code: Some(code.clone()),
    });
    let mut virtual_code = code;
    virtual_code
        .set_register_frame(2, 1, 0)
        .expect("virtual frame");
    virtual_code
        .replace_instruction(1, Instruction::ReturnObject { src: 0 })
        .expect("return edit");
    class.add_virtual_method(EncodedMethod {
        method: virtual_method,
        access_flags: AccessFlags::PUBLIC,
        code: Some(virtual_code),
    });
    dex
}

pub fn direct_method(dex: &mut DexFile) -> &mut EncodedMethod {
    &mut dex
        .class_mut(0)
        .expect("fixture class")
        .class_data
        .as_mut()
        .expect("fixture members")
        .direct_methods[0]
}

pub fn body(dex: &mut DexFile) -> &mut CodeItem {
    direct_method(dex)
        .code
        .as_mut()
        .expect("fixture method body")
}

pub fn parse_with_loading(
    bytes: &[u8],
    classes: reseam_dex::Loading,
) -> reseam_dex::Result<DexFile> {
    reseam_dex::parse(
        bytes,
        reseam_dex::ParseOptions {
            classes,
            ..Default::default()
        },
    )
}

pub fn unchecked() -> reseam_dex::ParseOptions {
    reseam_dex::ParseOptions {
        checksum: reseam_dex::Verification::Skip,
        signature: reseam_dex::Verification::Skip,
        ..Default::default()
    }
}
