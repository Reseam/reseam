// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::*;

fn metadata_fixture() -> DexFile {
    use crate::model::debug::{DebugBytecode, DebugInfo};
    use crate::model::metadata::Metadata;
    let mut dex = DexFile::new(DexHeader::new(crate::DexVersion::V035));
    let class = dex
        .create_class("LMetadata;", AccessFlags::PUBLIC, None)
        .unwrap();
    let method = dex.intern_method("LMetadata;", "run", "()V").unwrap();
    dex.class_mut(class).unwrap().annotations = Some(Box::new(Metadata::new(
        crate::AnnotationsDirectory::default(),
    )));
    dex.class_mut(class)
        .unwrap()
        .add_method(crate::EncodedMethod {
            method,
            access_flags: AccessFlags::STATIC,
            code: Some(std::sync::Arc::new(crate::CodeItem {
                registers_size: 0,
                ins_size: 0,
                outs_size: 0,
                instructions: vec![crate::Instruction::ReturnVoid],
                tries: Vec::new(),
                catch_handlers: Vec::new(),
                debug_info: Some(Metadata::new(DebugInfo {
                    line_start: 42,
                    parameter_names: Vec::new(),
                    bytecodes: vec![
                        DebugBytecode::SpecialAdvance {
                            line_advance: 0,
                            pc_advance: 0,
                        },
                        DebugBytecode::EndSequence,
                    ],
                })),
            })),
        });
    dex
}

#[test]
fn deferred_metadata_survives_resolution_and_serialization_policies_are_explicit() {
    use crate::options::Loading;
    use crate::options::{MetadataPolicy, WriteOptions};
    let bytes = write(&metadata_fixture()).unwrap();
    for lazy in [false, true] {
        let mut dex = crate::parse(
            &bytes,
            ParseOptions {
                classes: if lazy {
                    Loading::Deferred
                } else {
                    Loading::Eager
                },
                debug_info: Loading::Deferred,
                annotations: Loading::Deferred,
                ..Default::default()
            },
        )
        .unwrap();
        for class in 0..dex.classes().len() {
            dex.class_mut(class).unwrap();
        }
        let output = write(&dex).unwrap();
        let parsed = crate::parse(&output, ParseOptions::default()).unwrap();
        assert!(parsed.classes().resident(0).unwrap().annotations.is_some());
        assert_eq!(
            parsed
                .classes()
                .resident(0)
                .unwrap()
                .class_data
                .as_ref()
                .unwrap()
                .direct_methods[0]
                .code
                .as_ref()
                .unwrap()
                .debug_info
                .as_ref()
                .unwrap()
                .read()
                .unwrap()
                .line_start,
            42
        );
        dex.set_write_options(WriteOptions {
            debug_info: MetadataPolicy::Omit,
            annotations: MetadataPolicy::Omit,
            ..Default::default()
        });
        let omitted = crate::parse(&write(&dex).unwrap(), ParseOptions::default()).unwrap();
        let class = omitted.classes().resident(0).unwrap();
        assert!(class.annotations.is_none());
        assert!(
            class.class_data.as_ref().unwrap().direct_methods[0]
                .code
                .as_ref()
                .unwrap()
                .debug_info
                .is_none()
        );
    }
}

#[test]
fn opaque_link_data_requires_an_explicit_omission_policy() {
    use crate::options::{LinkPolicy, WriteOptions};
    let mut bytes = write(&DexFile::new(DexHeader::new(crate::DexVersion::V035))).unwrap();
    let offset = bytes.len() as u32;
    bytes.extend_from_slice(b"opaque");
    let size = bytes.len() as u32;
    bytes[0x20..0x24].copy_from_slice(&size.to_le_bytes());
    bytes[0x2c..0x30].copy_from_slice(&6u32.to_le_bytes());
    bytes[0x30..0x34].copy_from_slice(&offset.to_le_bytes());
    let options = ParseOptions {
        checksum: crate::options::Verification::Skip,
        signature: crate::options::Verification::Skip,
        ..Default::default()
    };
    let mut dex = crate::parse(&bytes, options).unwrap();
    assert_eq!(dex.link_data(), Some(b"opaque".as_slice()));
    assert!(write(&dex).is_err());
    dex.set_write_options(WriteOptions {
        link_data: LinkPolicy::Omit,
        ..Default::default()
    });
    let written = write(&dex).unwrap();
    assert!(
        crate::parse(&written, ParseOptions::default())
            .unwrap()
            .link_data()
            .is_none()
    );
    assert!(crate::parse(&bytes[..bytes.len() - 1], options).is_err());
}
