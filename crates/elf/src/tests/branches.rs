// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Checks of branch lookups and edits, run on each machine's branch fixture by `a64` and `arm`.

use std::ops::Range;

use pretty_assertions::assert_eq;

use super::{editor, pattern_at, written};
use crate::{Branch, Editor, ElfError, InstructionSet, Query};

/// A conditional branch of the branch fixtures: the function exporting it, the branch's offset in
/// it, its size, the offset in the function it jumps to, and the code that makes it always and
/// never jump, from the NDK assembler.
pub(super) struct BranchCase {
    pub(super) function: &'static str,
    pub(super) offset: u64,
    pub(super) size: u64,
    pub(super) target: u64,
    pub(super) always: Vec<u8>,
    pub(super) never: Vec<u8>,
}

/// An edit of a branch: [`Editor::always_take`] or [`Editor::never_take`].
type BranchEdit = fn(&mut Editor, Branch) -> crate::Result<()>;

/// Checks that a pattern starting in the instruction before each branch and ending with it finds
/// the branch, and that each edit replaces exactly the branch.
pub(super) fn assert_branch_edits(
    library: &[u8],
    instruction_set: InstructionSet,
    cases: &[BranchCase],
) {
    let original = editor(library);
    for case in cases {
        let query = Query {
            symbol: Some(case.function),
            ..Query::default()
        };
        let function = original.find_function(&query).expect("one match");
        let address = function.address + case.offset;
        let pattern = pattern_at(library, address - 2, case.size as usize + 2);
        let branch = original
            .find_branch(function, &pattern)
            .expect("one branch");
        assert_eq!(
            branch,
            Branch {
                address,
                size: case.size,
                target: function.address + case.target,
                instruction_set,
            },
            "{}",
            case.function
        );
        let edits: [(BranchEdit, &[u8]); 2] = [
            (Editor::always_take, &case.always),
            (Editor::never_take, &case.never),
        ];
        for (edit, code) in edits {
            let mut edited = editor(library);
            edit(&mut edited, branch).expect("edit fits");
            let mut expected = library.to_vec();
            expected[address as usize..][..code.len()].copy_from_slice(code);
            assert_eq!(written(&edited), expected, "{}", case.function);
        }
    }
}

/// Checks that a pattern of the bytes at `twice` in `function`, which cover two conditional
/// branches, and one of the bytes at `none`, which cover none, find no branch.
pub(super) fn assert_branch_ambiguity(
    library: &[u8],
    function: &str,
    twice: Range<u64>,
    none: Range<u64>,
) {
    let editor = editor(library);
    let query = Query {
        symbol: Some(function),
        ..Query::default()
    };
    let function = editor.find_function(&query).expect("one match");
    for (bytes, expected) in [(twice, 2), (none, 0)] {
        let pattern = pattern_at(
            library,
            function.address + bytes.start,
            (bytes.end - bytes.start) as usize,
        );
        match editor.find_branch(function, &pattern) {
            Err(ElfError::Branch { count, .. }) => assert_eq!(count, expected, "{pattern}"),
            other => panic!("{pattern} found {other:?}"),
        }
    }
}
