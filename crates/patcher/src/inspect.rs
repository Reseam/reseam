// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Kotlin run against an APK outside a patch: a target expression, to see
//! what it matches, or a snippet with the patch API in scope. Both compile
//! with a Kotlin compiler the host provides and run in a patch runtime over
//! the APK, so a target matches exactly what it matches in a patch.

#[cfg(all(feature = "kotlin", not(target_os = "android")))]
use reseam_model::analysis::{CompileError, Snippet};

#[cfg(all(feature = "kotlin", not(target_os = "android")))]
pub use crate::bridge::runtime::inspector::inspect;

/// The Kotlin version the patch runtime is compiled with. The compiler a host
/// provides for [`inspect`] must be this version.
#[cfg(all(feature = "kotlin", not(target_os = "android")))]
pub const KOTLIN_VERSION: &str = env!("RESEAM_KOTLIN_VERSION");
#[cfg(all(feature = "kotlin", not(target_os = "android")))]
pub use crate::bridge::runtime::target_check::{CheckedTargets, TargetCheck};

/// The generated file around a snippet, split where the snippet goes.
#[cfg(all(feature = "kotlin", not(target_os = "android")))]
pub(crate) fn source(snippet: &Snippet) -> (String, u32) {
    const HEADER: &str = "package app.reseam.inspection\n\nimport app.reseam.patch.*\nimport app.reseam.patch.dex.*\nimport app.reseam.patch.types.*\n\nobject Snippet {\n";
    let (open, text, close) = match snippet {
        Snippet::Target(text) => (
            "    @JvmStatic\n    fun target(): app.reseam.patch.Target<*> =\n",
            text,
            "\n}\n",
        ),
        Snippet::Code(text) => (
            "    @JvmStatic\n    fun run(runtime: app.reseam.patch.PatchRuntime) {\n        with(runtime) {\n",
            text,
            "\n        }\n    }\n}\n",
        ),
    };
    let before = format!("{HEADER}{open}");
    let offset = u32::try_from(before.lines().count()).expect("the header is a few lines");
    (format!("{before}{text}{close}"), offset)
}

/// The errors in the compiler's messages, at snippet lines: each starts
/// `<file>:<line>:<column>: error: <message>`.
#[cfg(all(feature = "kotlin", not(target_os = "android")))]
pub(crate) fn compile_errors(messages: &str, offset: u32) -> Vec<CompileError> {
    messages
        .lines()
        .filter_map(|line| {
            let (location, message) = line.split_once(": error: ")?;
            let mut parts = location.rsplitn(3, ':');
            let column = parts.next()?.parse().ok()?;
            let line: u32 = parts.next()?.parse().ok()?;
            Some(CompileError {
                line: line.saturating_sub(offset),
                column,
                message: message.to_owned(),
            })
        })
        .collect()
}

#[cfg(all(test, feature = "kotlin", not(target_os = "android")))]
mod tests {
    use reseam_model::analysis::{CompileError, Snippet};

    use super::{compile_errors, source};

    #[test]
    fn compiler_errors_point_at_snippet_lines() {
        let (text, offset) = source(&Snippet::Target("method {\n    strng(\"a\")\n}".to_owned()));
        let line = text
            .lines()
            .position(|line| line.contains("strng"))
            .expect("in the file")
            + 1;
        let messages = format!(
            "/tmp/x/Snippet.kt:{line}:5: error: unresolved reference 'strng'.\nwarning: ignored"
        );

        assert_eq!(
            compile_errors(&messages, offset),
            [CompileError {
                line: 2,
                column: 5,
                message: "unresolved reference 'strng'.".to_owned(),
            }]
        );
    }
}
