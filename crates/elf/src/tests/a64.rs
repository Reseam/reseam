// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::branches::{BranchCase, assert_branch_ambiguity, assert_branch_edits};
use super::{AARCH64, assert_returns, editor, finds, function};
use crate::InstructionSet::A64;

/// The first function of `fixtures/registers.S`, which starts with `bti c`.
const KEPT_BY_STORE: crate::Function = function(0xd74, 0x14, A64);

/// Each string's page goes through one instruction before its offset is added; see
/// `fixtures/registers.S`.
#[test]
fn string_references_survive_only_instructions_that_keep_the_register() {
    let editor = editor(AARCH64.bytes);
    let cases: &[(&str, bool)] = &[
        ("store keeps the page", true),
        ("pre-indexed pair store keeps the page", true),
        ("vector load keeps the page", true),
        ("move clears the page", false),
        ("call clears the page", false),
        ("literal load clears the page", false),
        ("atomic add clears the page", false),
        ("exclusive store status clears the page", false),
        ("system register read clears the page", false),
    ];
    for &(text, found) in cases {
        assert_eq!(finds(&editor, text), found, "{text}");
    }
}

#[test]
fn always_return_keeps_the_landing_pad_and_authenticates_a_signed_return() {
    const PACIASP: u32 = 0xd503_233f;
    const AUTIASP: u32 = 0xd503_23bf;
    const BTI_C: u32 = 0xd503_245f;
    const RET: u32 = 0xd65f_03c0;
    let has_access = AARCH64.has_access;
    let cases: &[(crate::Function, i32, &[u32])] = &[
        (has_access, 1, &[PACIASP, 0x5280_0020, AUTIASP, RET]),
        (has_access, 0, &[PACIASP, 0x5280_0000, AUTIASP, RET]),
        (has_access, -1, &[PACIASP, 0x1280_0000, AUTIASP, RET]),
        (has_access, 0x1_0000, &[PACIASP, 0x52a0_0020, AUTIASP, RET]),
        (
            has_access,
            0x1234_5678,
            &[PACIASP, 0x528a_cf00, 0x72a2_4680, AUTIASP, RET],
        ),
        (KEPT_BY_STORE, 1, &[BTI_C, 0x5280_0020, RET]),
    ];
    for &(function, value, code) in cases {
        let code: Vec<u8> = code.iter().copied().flat_map(u32::to_le_bytes).collect();
        assert_returns(AARCH64.bytes, function, value, &code);
    }
}

/// The branches of `fixtures/branches.S`: each function starts with `bti c` and an instruction
/// before the branch.
#[test]
fn branch_edits_jump_always_or_never() {
    const NOP: u32 = 0xd503_201f;
    let forward = |function| BranchCase {
        function,
        offset: 8,
        size: 4,
        target: 20,
        always: 0x1400_0003_u32.to_le_bytes().to_vec(),
        never: NOP.to_le_bytes().to_vec(),
    };
    let cases = [
        forward("branch_ne"),
        forward("branch_bc"),
        forward("branch_cbz"),
        forward("branch_cbnz"),
        forward("branch_tbz"),
        forward("branch_tbnz"),
        BranchCase {
            function: "branch_back",
            offset: 20,
            size: 4,
            target: 8,
            always: 0x17ff_fffd_u32.to_le_bytes().to_vec(),
            never: NOP.to_le_bytes().to_vec(),
        },
    ];
    assert_branch_edits(AARCH64.bytes, A64, &cases);
    assert_branch_ambiguity(AARCH64.bytes, "branch_twice", 8..16, 16..24);
}
