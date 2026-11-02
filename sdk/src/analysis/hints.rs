// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Names the DEX still holds for obfuscated classes and methods.

use std::collections::BTreeSet;

use reseam_dex::{ClassDef, CodeItem, DexFile, StringIdx, reaching_definitions};
use reseam_model::analysis::{ClassRef, HintSource, MethodRef, NameHint};

use super::Analysis;
use super::code::{body, method_ref};
use crate::error::Result;

const LOG: &str = "Landroid/util/Log;";
const LOG_LEVELS: [&str; 6] = ["v", "d", "i", "w", "e", "wtf"];
const INTRINSICS: &str = "Lkotlin/jvm/internal/Intrinsics;";
const PARAMETER_CHECKS: [&str; 2] = ["checkNotNullParameter", "checkParameterIsNotNull"];

impl Analysis {
    /// Names for `class`: its source file, its `toString` prefix and its log tags.
    pub fn class_hints(&self, class: &ClassRef) -> Result<Vec<NameHint>> {
        let (dex, def) = self.class(class)?;
        Ok(class_hints(dex, class, &def))
    }

    /// Parameter names from Kotlin's null checks in `method`'s body.
    pub(super) fn method_hints(&self, method: &MethodRef) -> Result<Vec<NameHint>> {
        let (dex, class, idx) = self.method(method)?;
        Ok(body(&class, idx).map_or_else(Vec::new, |code| parameter_hints(dex, code)))
    }
}

pub(super) fn class_hints(dex: &DexFile, class: &ClassRef, def: &ClassDef) -> Vec<NameHint> {
    let mut hints = BTreeSet::new();
    if let Some(stem) = def
        .source_file
        .map(|file| dex.strings().get(file))
        .and_then(|file| {
            let file = file.as_ref();
            file.strip_suffix(".java")
                .or_else(|| file.strip_suffix(".kt"))
                .map(str::to_owned)
        })
        .filter(|stem| stem != class.simple_name())
    {
        hints.insert(hint(stem, HintSource::SourceFile));
    }
    let Some(data) = def.class_data.as_deref() else {
        return hints.into_iter().collect();
    };
    for method in data.direct_methods.iter().chain(&data.virtual_methods) {
        let Some(code) = method.code() else {
            continue;
        };
        let reference = method_ref(dex, method.method);
        if reference.name == "toString" && reference.proto == "()Ljava/lang/String;" {
            hints.extend(to_string_hint(dex, code));
        }
        for tag in string_arguments(dex, code, LOG, &LOG_LEVELS, 0) {
            if is_identifier(&tag) {
                hints.insert(hint(tag, HintSource::LogTag));
            }
        }
    }
    hints.into_iter().collect()
}

fn parameter_hints(dex: &DexFile, code: &CodeItem) -> Vec<NameHint> {
    let names: BTreeSet<String> = string_arguments(dex, code, INTRINSICS, &PARAMETER_CHECKS, 1)
        .into_iter()
        .filter(|name| is_identifier(name))
        .collect();
    names
        .into_iter()
        .map(|name| hint(name, HintSource::ParameterCheck))
        .collect()
}

/// The class name a data class's `toString` starts with: `Name(` or `Name{`.
fn to_string_hint(dex: &DexFile, code: &CodeItem) -> Option<NameHint> {
    code.instructions().iter().find_map(|instruction| {
        let text = dex.strings().get(instruction.string_ref()?);
        let name = text.strip_suffix(['(', '{']).filter(|name| {
            name.starts_with(|c: char| c.is_ascii_uppercase()) && is_identifier(name)
        })?;
        Some(hint(name.to_owned(), HintSource::ToString))
    })
}

/// The constant string each call to a static method of `owner` named one of
/// `names` passes as argument word `word`, when only that constant reaches it.
fn string_arguments(
    dex: &DexFile,
    code: &CodeItem,
    owner: &str,
    names: &[&str],
    word: usize,
) -> Vec<String> {
    code.instructions()
        .iter()
        .enumerate()
        .filter_map(|(index, instruction)| {
            let id = dex.method_id(instruction.method_ref()?);
            let called = dex.strings().get(id.name);
            if dex.type_descriptor(id.class) != owner || !names.contains(&called.as_ref()) {
                return None;
            }
            let string = constant_string(code, index, instruction.argument_register(word)?)?;
            Some(dex.strings().get(string).into_owned())
        })
        .collect()
}

/// The string `register` holds at instruction `index`, when its only
/// definition reaching there is a `const-string`.
fn constant_string(code: &CodeItem, index: usize, register: u16) -> Option<StringIdx> {
    let (definitions, from_entry) = reaching_definitions(code, index, register)?;
    let [definition] = definitions[..] else {
        return None;
    };
    if from_entry {
        return None;
    }
    code.instructions()[definition].string_ref()
}

fn is_identifier(text: &str) -> bool {
    text.starts_with(|c: char| c.is_alphabetic() || c == '_')
        && text
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

fn hint(name: String, source: HintSource) -> NameHint {
    NameHint { name, source }
}
