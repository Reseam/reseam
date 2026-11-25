// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `fixtures/library.c` built for each machine, and lookups and edits that work the same on both.
//! The machine-specific cases are in `a64` and `arm`.

mod a64;
mod arm;
mod branches;
mod pattern;

use pretty_assertions::assert_eq;
use reseam_journal::Savepoint;
use reseam_storage::Bytes;

use crate::InstructionSet::{A32, A64, T32};
use crate::{Editor, ElfError, Function, InstructionSet, Pattern, Query};

/// The functions of `fixtures/library.c` in one build. A function found by a string ends with
/// its last instruction; the symbol of `exported_check` gives its size with the literals after.
struct Library {
    bytes: &'static [u8],
    exported_check: Function,
    has_access: Function,
    shared_a: Function,
    level: Function,
    shared_b: Function,
    pointed: Function,
    subscribed: Function,
}

const fn function(address: u64, size: u64, instruction_set: InstructionSet) -> Function {
    Function {
        address,
        size,
        instruction_set,
    }
}

/// Built with NDK 29: `clang --target=aarch64-linux-android21 -O2 -fPIC -shared
/// -mbranch-protection=standard library.c registers.S branches.S`, then `llvm-strip`.
const AARCH64: Library = Library {
    bytes: include_bytes!("../../fixtures/library.so"),
    exported_check: function(0xc40, 0x50, A64),
    has_access: function(0xca0, 0x2c, A64),
    shared_a: function(0xccc, 0x28, A64),
    level: function(0xcf4, 0x40, A64),
    shared_b: function(0xd34, 0x40, A64),
    pointed: function(0xc30, 0x10, A64),
    subscribed: function(0xc90, 0x10, A64),
};

/// Built with NDK 29 and the flags the NDK passes for armeabi-v7a: `clang
/// --target=armv7a-linux-androideabi21 -mthumb -funwind-tables -ffunction-sections
/// -fdata-sections -O2 -fPIC -shared library.c registers-arm.S branches-arm.S`, then `llvm-strip`. The linker
/// merges the unwind entries of `subscribed`, `has_access` and `shared_a`, and `level` is A32.
const ARM: Library = Library {
    bytes: include_bytes!("../../fixtures/library-arm.so"),
    exported_check: function(0x1214, 0x2c, T32),
    has_access: function(0x1254, 0x16, T32),
    shared_a: function(0x1270, 0x10, T32),
    level: function(0x1284, 0x2c, A32),
    shared_b: function(0x12b8, 0x1a, T32),
    pointed: function(0x1200, 0x10, T32),
    subscribed: function(0x1240, 0x10, T32),
};

const LIBRARIES: [&Library; 2] = [&AARCH64, &ARM];

fn editor(bytes: &[u8]) -> Editor {
    Editor::new(Bytes::from_slice(bytes)).expect("fixture parses")
}

fn written(editor: &Editor) -> Vec<u8> {
    let mut output = Vec::new();
    editor.write(&mut output).expect("library writes");
    output
}

/// Whether a lookup of `text` alone finds one function, failing on any other error.
/// A pattern of the `len` bytes at `address` of `library`, the first one unknown.
fn pattern_at(library: &[u8], address: u64, len: usize) -> Pattern {
    let bytes = &library[address as usize..][..len];
    let known = bytes[1..].iter().map(|byte| format!("{byte:02x}"));
    std::iter::once("??".to_owned())
        .chain(known)
        .collect::<Vec<_>>()
        .join(" ")
        .parse()
        .expect("pattern parses")
}

fn finds(editor: &Editor, text: &str) -> bool {
    let query = Query {
        strings: &[text],
        ..Query::default()
    };
    match editor.find_function(&query) {
        Ok(_) => true,
        Err(ElfError::Match { count: 0, .. }) => false,
        other => panic!("{text}: {other:?}"),
    }
}

#[test]
fn finds_one_function_by_every_constraint() {
    for library in LIBRARIES {
        let editor = editor(library.bytes);
        let has_access_code = [pattern_at(
            library.bytes,
            library.has_access.address + 8,
            12,
        )];
        let shared_b = library.shared_b;
        let shared_b_code = [pattern_at(
            library.bytes,
            shared_b.address,
            shared_b.size as usize,
        )];
        let cases = [
            (
                Query {
                    symbol: Some("exported_check"),
                    ..Query::default()
                },
                library.exported_check,
            ),
            (
                Query {
                    strings: &["2026-11-access-check"],
                    ..Query::default()
                },
                library.has_access,
            ),
            (
                Query {
                    strings: &["feature-level-bonus"],
                    ..Query::default()
                },
                library.level,
            ),
            (
                Query {
                    strings: &["shared-key", "only-in-b"],
                    ..Query::default()
                },
                library.shared_b,
            ),
            (
                Query {
                    strings: &["shared-key"],
                    nth: Some(0),
                    ..Query::default()
                },
                library.shared_a,
            ),
            (
                Query {
                    strings: &["shared-key"],
                    nth: Some(1),
                    ..Query::default()
                },
                library.shared_b,
            ),
            (
                Query {
                    strings: &["reached through a pointer"],
                    ..Query::default()
                },
                library.pointed,
            ),
            (
                Query {
                    strings: &["reached through a callback"],
                    ..Query::default()
                },
                library.subscribed,
            ),
            (
                Query {
                    patterns: &has_access_code,
                    ..Query::default()
                },
                library.has_access,
            ),
            (
                Query {
                    strings: &["shared-key"],
                    patterns: &shared_b_code,
                    ..Query::default()
                },
                library.shared_b,
            ),
        ];
        for (query, expected) in cases {
            assert_eq!(
                editor.find_function(&query).expect("one match"),
                expected,
                "{query}"
            );
        }
    }
}

#[test]
fn reports_missing_and_ambiguous_matches() {
    for library in LIBRARIES {
        let editor = editor(library.bytes);
        let (shared_a, level) = (library.shared_a, library.level);
        let across_functions = [pattern_at(
            library.bytes,
            shared_a.address + shared_a.size - 2,
            (level.address + 2 - (shared_a.address + shared_a.size - 2)) as usize,
        )];
        let cases = [
            (
                Query {
                    strings: &["shared-key"],
                    ..Query::default()
                },
                2,
            ),
            (
                Query {
                    strings: &["shared-key"],
                    nth: Some(2),
                    ..Query::default()
                },
                2,
            ),
            (
                Query {
                    symbol: Some("exported_check"),
                    strings: &["shared-key"],
                    ..Query::default()
                },
                0,
            ),
            (
                Query {
                    symbol: Some("missing"),
                    ..Query::default()
                },
                0,
            ),
            (
                Query {
                    strings: &["not in the library"],
                    ..Query::default()
                },
                0,
            ),
            (
                Query {
                    patterns: &across_functions,
                    ..Query::default()
                },
                0,
            ),
            (Query::default(), 0),
        ];
        for (query, expected) in cases {
            match editor.find_function(&query) {
                Err(ElfError::Match { count, .. }) => assert_eq!(count, expected, "{query}"),
                other => panic!("{query} matched {other:?}"),
            }
        }
    }
}

#[test]
fn unedited_output_is_the_source() {
    for library in LIBRARIES {
        assert_eq!(written(&editor(library.bytes)), library.bytes);
    }
}

#[test]
fn rejects_edits_past_the_function() {
    for library in LIBRARIES {
        let function = library.has_access;
        let cases: &[(u64, u64)] = &[
            (function.size - 2, function.size + 2),
            (u64::MAX - 1, u64::MAX),
        ];
        for &(offset, needed) in cases {
            match editor(library.bytes).replace_bytes(function, offset, &[0; 4]) {
                Err(ElfError::TooSmall { needed: got, .. }) => assert_eq!(got, needed, "{offset}"),
                other => panic!("{offset}: {other:?}"),
            }
        }
    }
}

#[test]
fn rollback_undoes_edits_since_the_savepoint() {
    for library in LIBRARIES {
        let mut editor = editor(library.bytes);
        editor.always_return(library.level, 0).expect("edit fits");
        let kept = written(&editor);
        editor.open_savepoint(Savepoint::FIRST);
        editor
            .always_return(library.has_access, 1)
            .expect("edit fits");
        editor.rollback_to(Savepoint::FIRST);
        assert_eq!(written(&editor), kept);
    }
}

/// Checks that `always_return` writes `code` at the start of `function` and nothing else.
fn assert_returns(library: &[u8], function: Function, value: i32, code: &[u8]) {
    let mut editor = editor(library);
    editor.always_return(function, value).expect("edit fits");
    let output = written(&editor);
    let start = function.address as usize;
    let end = start + code.len();
    assert_eq!(&output[start..end], code, "{function:?} {value}");
    assert_eq!(output[..start], library[..start]);
    assert_eq!(output[end..], library[end..]);
}

#[test]
fn rejects_other_files() {
    assert!(matches!(
        Editor::new(Bytes::from_slice(b"not a library")),
        Err(ElfError::Magic)
    ));
}
