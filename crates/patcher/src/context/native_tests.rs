// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_apk::ApkFile;
use reseam_elf::{InstructionSet, Pattern};
use reseam_journal::Savepoint;
use reseam_model::Abi;

use super::{NativeQuery, PatchContext};
use crate::context::ExtensionSet;
use crate::error::PatcherError;
use crate::log::LogLevel;
use crate::test_support::{NATIVE_LIBRARY as LIBRARY, NATIVE_LIBRARY_ARM, apk_with};

const LIBRARY_ENTRY: &str = "lib/arm64-v8a/library.so";
const ARM_ENTRY: &str = "lib/armeabi-v7a/library.so";
/// An ABI's copy in a universal APK that cannot be edited.
const X86_ENTRY: &str = "lib/x86/library.so";
const X86_COPY: &[u8] = b"not an ARM elf, left as is";
/// `has_access` in the arm64-v8a test library, which starts with `paciasp`.
const HAS_ACCESS: usize = 0xca0;

const EXPORTED_CHECK: NativeQuery<'static> = NativeQuery {
    library: "library.so",
    abi: None,
    symbol: Some("exported_check"),
    strings: &[],
    patterns: &[],
    nth: None,
};

fn universal_apk(device: &[Abi]) -> (tempfile::TempDir, ApkFile) {
    apk_with(
        "<application/>",
        &[
            (LIBRARY_ENTRY, LIBRARY),
            (ARM_ENTRY, NATIVE_LIBRARY_ARM),
            (X86_ENTRY, X86_COPY),
        ],
        device,
    )
}

fn patched_library(edit: impl FnOnce(&mut PatchContext<'_>)) -> Vec<u8> {
    let (_dir, mut apk) = apk_with("<application/>", &[(LIBRARY_ENTRY, LIBRARY)], &[]);
    let mut ctx = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
    edit(&mut ctx);
    ctx.finish_native().unwrap();
    ctx.read_file(0, LIBRARY_ENTRY).unwrap().unwrap()
}

/// Makes `exported_check` return 1 in every copy the run edits.
fn edit_exported_check(ctx: &mut PatchContext<'_>) -> crate::error::Result<()> {
    let found = ctx.native_find(&EXPORTED_CHECK)?;
    ctx.native_always_return(&found, 1)
}

fn warnings(ctx: &mut PatchContext<'_>) -> Vec<String> {
    ctx.scope()
        .log
        .take_entries()
        .into_iter()
        .filter(|entry| entry.level == LogLevel::Warn)
        .map(|entry| entry.message)
        .collect()
}

#[test]
fn always_return_edits_the_library_in_place() {
    let output = patched_library(|ctx| {
        let found = ctx
            .native_find(&NativeQuery {
                library: "library.so",
                strings: &["2026-11-access-check"],
                ..NativeQuery::default()
            })
            .unwrap();
        assert_eq!(found.len(), 1);
        ctx.native_always_return(&found, 1).unwrap();
    });
    assert_eq!(output.len(), LIBRARY.len());
    // paciasp kept, then movz w0,#1; autiasp; ret.
    assert_eq!(
        output[HAS_ACCESS..HAS_ACCESS + 4],
        LIBRARY[HAS_ACCESS..HAS_ACCESS + 4]
    );
    assert_eq!(
        output[HAS_ACCESS + 4..HAS_ACCESS + 16],
        [
            0x20, 0x00, 0x80, 0x52, 0xbf, 0x23, 0x03, 0xd5, 0xc0, 0x03, 0x5f, 0xd6
        ]
    );
    assert_eq!(output[..HAS_ACCESS], LIBRARY[..HAS_ACCESS]);
}

/// The copies a run edits, or the device ABI it refuses.
type Edited = Result<&'static [&'static str], Abi>;

#[test]
fn a_run_edits_the_arm_copies_the_device_loads() {
    let cases: &[(&[Abi], Edited)] = &[
        (&[], Ok(&[LIBRARY_ENTRY, ARM_ENTRY])),
        (&[Abi::Arm64V8a, Abi::ArmeabiV7a], Ok(&[LIBRARY_ENTRY])),
        (&[Abi::ArmeabiV7a], Ok(&[ARM_ENTRY])),
        (&[Abi::X86], Err(Abi::X86)),
    ];
    for &(device, expected) in cases {
        let (_dir, mut apk) = universal_apk(device);
        let mut ctx = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
        match (edit_exported_check(&mut ctx), expected) {
            (Ok(()), Ok(edited)) => {
                ctx.finish_native().unwrap();
                for (entry, source) in [
                    (LIBRARY_ENTRY, LIBRARY),
                    (ARM_ENTRY, NATIVE_LIBRARY_ARM),
                    (X86_ENTRY, X86_COPY),
                ] {
                    let output = ctx.read_file(0, entry).unwrap().unwrap();
                    assert_eq!(
                        output != source,
                        edited.contains(&entry),
                        "{device:?} {entry}"
                    );
                }
            }
            (Err(PatcherError::NativeAbi(abi)), Err(refused)) => assert_eq!(abi, refused),
            (other, _) => panic!("{device:?}: {other:?}"),
        }
    }
}

#[test]
fn a_run_for_any_device_warns_about_copies_it_cannot_edit() {
    let (_dir, mut apk) = universal_apk(&[]);
    let mut ctx = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
    edit_exported_check(&mut ctx).unwrap();
    assert_eq!(
        warnings(&mut ctx),
        [format!(
            "{X86_ENTRY} stays unedited: x86 native code cannot be edited"
        )]
    );
}

/// A target narrowed to an ABI matches nothing when the run edits no copy for it.
#[test]
fn abi_narrows_a_target_to_one_copy() {
    let cases: &[(&[Abi], Abi, &[&str])] = &[
        (&[], Abi::ArmeabiV7a, &[ARM_ENTRY]),
        (&[], Abi::Arm64V8a, &[LIBRARY_ENTRY]),
        (&[Abi::Arm64V8a], Abi::ArmeabiV7a, &[]),
    ];
    for &(device, abi, expected) in cases {
        let (_dir, mut apk) = universal_apk(device);
        let mut ctx = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
        let found = ctx
            .native_find(&NativeQuery {
                abi: Some(abi),
                ..EXPORTED_CHECK
            })
            .unwrap();
        let entries: Vec<&str> = found.iter().map(|(key, _)| key.entry.as_str()).collect();
        assert_eq!(entries, expected, "{device:?} {abi:?}");
    }
}

#[test]
fn a_library_no_editable_abi_carries_is_missing() {
    let (_dir, mut apk) = apk_with("<application/>", &[(X86_ENTRY, X86_COPY)], &[]);
    let mut ctx = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
    match ctx.native_find(&NativeQuery {
        abi: Some(Abi::ArmeabiV7a),
        ..EXPORTED_CHECK
    }) {
        Err(PatcherError::MissingNativeLibrary { library, abis }) => {
            assert_eq!(library, "library.so");
            assert_eq!(abis, [Abi::Arm64V8a, Abi::ArmeabiV7a]);
        }
        other => panic!("{:?}", other.map(|found| found.len())),
    }
}

#[test]
fn raw_bytes_need_one_instruction_set() {
    let (_dir, mut apk) = universal_apk(&[]);
    let mut ctx = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
    let found = ctx.native_find(&EXPORTED_CHECK).unwrap();
    let sets: Vec<InstructionSet> = found
        .iter()
        .map(|(_, function)| function.instruction_set)
        .collect();
    assert_eq!(sets, [InstructionSet::A64, InstructionSet::T32]);
    assert!(matches!(
        ctx.native_replace_bytes(&found, 0, &[0; 4]),
        Err(PatcherError::MixedInstructionSets { first, second })
            if first == LIBRARY_ENTRY && second == ARM_ENTRY
    ));
    ctx.native_replace_bytes(&found[1..], 0, &[0; 4]).unwrap();
}

#[test]
fn an_edited_library_cannot_be_replaced_or_deleted() {
    let (_dir, mut apk) = apk_with("<application/>", &[(LIBRARY_ENTRY, LIBRARY)], &[]);
    let mut ctx = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
    edit_exported_check(&mut ctx).unwrap();
    assert!(matches!(
        ctx.delete_file(0, LIBRARY_ENTRY),
        Err(PatcherError::EditedNativeLibrary(entry)) if entry == LIBRARY_ENTRY
    ));
    assert!(matches!(
        ctx.inject_file(
            0,
            LIBRARY_ENTRY,
            b"replaced".to_vec(),
            reseam_apk::Compression::Stored
        ),
        Err(PatcherError::EditedNativeLibrary(_))
    ));
}

#[test]
fn a_library_only_looked_up_can_be_deleted() {
    let (_dir, mut apk) = apk_with("<application/>", &[(LIBRARY_ENTRY, LIBRARY)], &[]);
    let mut ctx = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
    ctx.native_find(&EXPORTED_CHECK).unwrap();
    ctx.delete_file(0, LIBRARY_ENTRY).unwrap();
    ctx.finish_native().unwrap();
    assert_eq!(ctx.read_file(0, LIBRARY_ENTRY).unwrap(), None);
}

#[test]
fn an_unedited_library_is_left_untouched() {
    let output = patched_library(|ctx| {
        ctx.native_find(&EXPORTED_CHECK).unwrap();
    });
    assert_eq!(output, LIBRARY);
}

#[test]
fn rollback_discards_a_native_edit() {
    let (_dir, mut apk) = apk_with("<application/>", &[(LIBRARY_ENTRY, LIBRARY)], &[]);
    let mut ctx = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
    let found = ctx.native_find(&EXPORTED_CHECK).unwrap();
    ctx.open_savepoint(Savepoint::FIRST);
    ctx.native_always_return(&found, 0).unwrap();
    ctx.rollback_to(Savepoint::FIRST);
    ctx.finish_native().unwrap();
    assert_eq!(ctx.read_file(0, LIBRARY_ENTRY).unwrap().unwrap(), LIBRARY);
}

fn pattern(text: &str) -> Pattern {
    text.parse().unwrap()
}

/// `branch_ne` of the arm64-v8a library and `thumb_branch_narrow` of the armeabi-v7a one, found by
/// the code around their branch, return 1 when it jumps.
#[test]
fn byte_patterns_find_a_function_and_its_branch_in_each_copy() {
    let (_dir, mut apk) = universal_apk(&[]);
    let mut ctx = PatchContext::new(&mut apk, ExtensionSet::app_runtime());
    let functions = ctx
        .native_find(&NativeQuery {
            library: "library.so",
            patterns: &[
                (
                    Abi::Arm64V8a,
                    pattern("1f 00 00 71 61 00 00 54 40 00 80 52"),
                ),
                (Abi::ArmeabiV7a, pattern("00 28 01 d1 02 20")),
            ],
            ..NativeQuery::default()
        })
        .unwrap();
    let starts: Vec<u64> = functions
        .iter()
        .map(|(_, function)| function.address)
        .collect();
    assert_eq!(starts, [0xe28, 0x14ec]);
    match ctx.native_find_branch(&functions, &[(Abi::Arm64V8a, pattern("61 00 ?? 54"))]) {
        Err(PatcherError::MissingBranchPattern { entry, abi }) => {
            assert_eq!((entry.as_str(), abi), (ARM_ENTRY, Abi::ArmeabiV7a));
        }
        other => panic!("{:?}", other.map(|found| found.len())),
    }
    let branches = ctx
        .native_find_branch(
            &functions,
            &[
                (Abi::Arm64V8a, pattern("61 00 ?? 54")),
                (Abi::ArmeabiV7a, pattern("01 d1")),
            ],
        )
        .unwrap();
    ctx.native_always_take(&branches).unwrap();
    ctx.finish_native().unwrap();
    // b +12, and b.n +6.
    for (entry, source, at, code) in [
        (LIBRARY_ENTRY, LIBRARY, 0xe30, &[0x03, 0x00, 0x00, 0x14][..]),
        (ARM_ENTRY, NATIVE_LIBRARY_ARM, 0x14ee, &[0x01, 0xe0]),
    ] {
        let mut expected = source.to_vec();
        expected[at..at + code.len()].copy_from_slice(code);
        assert_eq!(
            ctx.read_file(0, entry).unwrap().unwrap(),
            expected,
            "{entry}"
        );
    }
}
