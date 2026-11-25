// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use pretty_assertions::assert_eq;

use super::branches::{BranchCase, assert_branch_ambiguity, assert_branch_edits};
use super::{ARM, assert_returns, editor, finds, function};
use crate::InstructionSet::{A32, T32};
use crate::Query;

fn halfwords(code: &[u16]) -> Vec<u8> {
    code.iter().copied().flat_map(u16::to_le_bytes).collect()
}

/// Each string's offset goes through one instruction before the program counter is added; see
/// `fixtures/registers-arm.S`.
#[test]
fn string_references_survive_only_instructions_that_keep_the_register() {
    let editor = editor(ARM.bytes);
    let cases: &[(&str, bool)] = &[
        ("thumb store keeps the offset", true),
        ("thumb compare keeps the offset", true),
        ("thumb vector load keeps the offset", true),
        ("thumb move clears the offset", false),
        ("thumb call clears the offset", false),
        ("thumb load clears the offset", false),
        ("thumb load multiple clears the offset", false),
        ("thumb load pair clears the offset", false),
        ("thumb long multiply clears the offset", false),
        ("thumb exclusive store clears the offset", false),
        ("thumb vector move clears the offset", false),
        ("thumb pop clears the offset", false),
        ("thumb wide move clears the offset", false),
        ("thumb shift clears the offset", false),
        ("thumb bitfield clears the offset", false),
        ("thumb system register read clears the offset", false),
        ("arm store keeps the offset", true),
        ("arm compare keeps the offset", true),
        ("arm move clears the offset", false),
        ("arm call clears the offset", false),
        ("arm load clears the offset", false),
        ("arm load pair clears the offset", false),
        ("arm multiply clears the offset", false),
        ("arm long multiply clears the offset", false),
        ("arm exclusive store clears the offset", false),
        ("arm vector move clears the offset", false),
        ("arm extend clears the offset", false),
        ("arm load multiple clears the offset", false),
        ("arm system register read clears the offset", false),
        ("thumb halves reach the string", true),
        ("arm halves reach the string", true),
        ("thumb added immediate reaches the string", true),
    ];
    for &(text, found) in cases {
        assert_eq!(finds(&editor, text), found, "{text}");
    }
}

/// The library linked with each form of relative relocations, which hold the only pointer to
/// `pointed`. The packed and RELR builds add `-Wl,--pack-dyn-relocs=android` and
/// `-Wl,--pack-dyn-relocs=relr -Wl,--use-android-relr-tags`.
#[test]
fn finds_functions_through_every_relocation_form() {
    let cases: &[(&[u8], u64)] = &[
        (ARM.bytes, 0x1200),
        (
            include_bytes!("../../fixtures/library-arm-packed.so"),
            0x11f0,
        ),
        (include_bytes!("../../fixtures/library-arm-relr.so"), 0x11e8),
    ];
    for &(bytes, address) in cases {
        let query = Query {
            strings: &["reached through a pointer"],
            ..Query::default()
        };
        assert_eq!(
            editor(bytes).find_function(&query).expect("one match"),
            function(address, 0x10, T32)
        );
    }
}

#[test]
fn always_return_writes_code_of_the_function_s_instruction_set() {
    const BX_LR_T32: u16 = 0x4770;
    const BX_LR_A32: u32 = 0xe12f_ff1e;
    let arm =
        |words: &[u32]| -> Vec<u8> { words.iter().copied().flat_map(u32::to_le_bytes).collect() };
    let cases: &[(crate::Function, i32, Vec<u8>)] = &[
        (ARM.has_access, 1, halfwords(&[0x2001, BX_LR_T32])),
        (ARM.has_access, 0, halfwords(&[0x2000, BX_LR_T32])),
        (ARM.has_access, -1, halfwords(&[0xf06f, 0x0000, BX_LR_T32])),
        (
            ARM.has_access,
            -256,
            halfwords(&[0xf06f, 0x00ff, BX_LR_T32]),
        ),
        (ARM.has_access, 300, halfwords(&[0xf240, 0x102c, BX_LR_T32])),
        (
            ARM.has_access,
            0x1_0000,
            halfwords(&[0xf240, 0x0000, 0xf2c0, 0x0001, BX_LR_T32]),
        ),
        (
            ARM.has_access,
            0x1234_5678,
            halfwords(&[0xf245, 0x6078, 0xf2c1, 0x2034, BX_LR_T32]),
        ),
        (ARM.level, 1, arm(&[0xe300_0001, BX_LR_A32])),
        (ARM.level, -1, arm(&[0xe3e0_0000, BX_LR_A32])),
        (
            ARM.level,
            0x1_0000,
            arm(&[0xe300_0000, 0xe340_0001, BX_LR_A32]),
        ),
        (
            ARM.level,
            0x1234_5678,
            arm(&[0xe305_0678, 0xe341_0234, BX_LR_A32]),
        ),
    ];
    for (function, value, code) in cases {
        assert_returns(ARM.bytes, *function, *value, code);
    }
}

/// The branches of `fixtures/branches-arm.S`. Each branch follows another instruction; in the
/// functions branching back, it follows a branch over the code it jumps to.
#[test]
fn branch_edits_jump_always_or_never() {
    const NOP_T32: u16 = 0xbf00;
    const NOP_WIDE_T32: [u16; 2] = [0xf3af, 0x8000];
    const NOP_A32: u32 = 0xe320_f000;
    let narrow = |function, offset, target, always| BranchCase {
        function,
        offset,
        size: 2,
        target,
        always: halfwords(&[always]),
        never: halfwords(&[NOP_T32]),
    };
    let wide = |function, offset, target, always: [u16; 2]| BranchCase {
        function,
        offset,
        size: 4,
        target,
        always: halfwords(&always),
        never: halfwords(&NOP_WIDE_T32),
    };
    let thumb = [
        narrow("thumb_branch_narrow", 2, 8, 0xe001),
        narrow("thumb_branch_cbz", 2, 8, 0xe001),
        narrow("thumb_branch_cbnz", 2, 8, 0xe001),
        narrow("thumb_branch_narrow_back", 8, 2, 0xe7fb),
        wide("thumb_branch_wide", 2, 10, [0xf000, 0xb802]),
        wide("thumb_branch_wide_back", 8, 2, [0xf7ff, 0xbffb]),
    ];
    assert_branch_edits(ARM.bytes, T32, &thumb);
    let arm = |function, offset, target, always: u32| BranchCase {
        function,
        offset,
        size: 4,
        target,
        always: always.to_le_bytes().to_vec(),
        never: NOP_A32.to_le_bytes().to_vec(),
    };
    assert_branch_edits(
        ARM.bytes,
        A32,
        &[
            arm("arm_branch", 4, 16, 0xea00_0001),
            arm("arm_branch_back", 16, 4, 0xeaff_fffb),
        ],
    );
    assert_branch_ambiguity(ARM.bytes, "thumb_branch_twice", 2..6, 6..10);
}
