// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Instructions found through the reference index, as code sites with smali.

use std::collections::BTreeMap;

use reseam_dex::{
    ClassDef, CodeItem, DexFile, EncodedMethod, FieldIdx, Instruction, InstructionRef, MethodIdx,
    RefQuery, SmaliLine, disassemble,
};
use reseam_model::analysis::{CodeSite, FieldRef, Member, MethodRef, RefKind};

use crate::error::Result;

/// The matching instructions of one method, by index.
pub(super) struct MethodHits<T> {
    pub(super) class_idx: usize,
    pub(super) method: MethodIdx,
    pub(super) hits: Vec<(u32, T)>,
}

/// Every instruction `matches` accepts, in the methods the index finds for
/// `query`; an empty query visits every method.
pub(super) fn scan<T: Send>(
    dex: &DexFile,
    query: &RefQuery,
    matches: impl Fn(InstructionRef<'_>) -> Option<T> + Sync,
) -> Result<Vec<MethodHits<T>>> {
    Ok(dex.scan_methods_collect(query, |view| {
        let mut hits = Vec::new();
        let mut index = 0u32;
        view.for_each_instruction(|instruction| {
            if let Some(hit) = matches(instruction) {
                hits.push((index, hit));
            }
            index += 1;
            true
        })?;
        Ok((!hits.is_empty()).then_some(MethodHits {
            class_idx: view.class_idx,
            method: view.method,
            hits,
        }))
    })?)
}

/// Each hit as a code site with its instruction in smali; each class is
/// decoded once however many of its methods hit.
pub(super) fn sites<T>(dex: &DexFile, hits: Vec<MethodHits<T>>) -> Result<Vec<(CodeSite, T)>> {
    let mut by_class: BTreeMap<usize, Vec<MethodHits<T>>> = BTreeMap::new();
    for method in hits {
        by_class.entry(method.class_idx).or_default().push(method);
    }
    let mut sites = Vec::new();
    for (class_idx, methods) in by_class {
        let class = dex.classes().read(class_idx, dex.parse_options())?;
        for method in methods {
            let reference = method_ref(dex, method.method);
            let Some(code) = body(&class, method.method) else {
                continue;
            };
            let lines = instruction_texts(dex, code)?;
            for (index, hit) in method.hits {
                let excerpt = lines
                    .get(&index)
                    .expect("every instruction of the body has a line")
                    .clone();
                sites.push((
                    CodeSite {
                        method: reference.clone(),
                        index,
                        excerpt,
                    },
                    hit,
                ));
            }
        }
    }
    Ok(sites)
}

/// `method` as `class` defines it.
pub(super) fn encoded_method(class: &ClassDef, method: MethodIdx) -> Option<&EncodedMethod> {
    let data = class.class_data.as_deref()?;
    let (kind, position) = data.find_method(method)?;
    Some(&data.methods(kind)[position])
}

/// The body of `method` as `class` defines it.
pub(super) fn body(class: &ClassDef, method: MethodIdx) -> Option<&CodeItem> {
    encoded_method(class, method)?.code()
}

/// The first line of each instruction's smali, by index.
fn instruction_texts(dex: &DexFile, code: &CodeItem) -> Result<BTreeMap<u32, String>> {
    Ok(disassemble(dex, code)?
        .into_iter()
        .filter_map(|line| match line {
            SmaliLine::Instruction { index, text } => Some((
                u32::try_from(index).expect("method bodies hold fewer than 2^32 instructions"),
                text.lines()
                    .next()
                    .expect("an instruction's text starts with its mnemonic")
                    .to_owned(),
            )),
            SmaliLine::Label(_) | SmaliLine::Directive(_) => None,
        })
        .collect())
}

/// The method, field or class an instruction names; a type names the class
/// it holds, through array dimensions, and a primitive type names none.
pub(super) fn named_member(dex: &DexFile, instruction: &Instruction) -> Option<Member> {
    if let Some(method) = instruction.method_ref() {
        return Some(Member::Method(method_ref(dex, method)));
    }
    if let Some(field) = instruction.field_ref() {
        return Some(Member::Field(field_ref(dex, field)));
    }
    dex.type_descriptor(instruction.type_ref()?)
        .trim_start_matches('[')
        .parse()
        .ok()
        .map(Member::Class)
}

pub(super) fn field_ref(dex: &DexFile, field: FieldIdx) -> FieldRef {
    dex.field_descriptor(field)
        .parse()
        .expect("the DEX writes valid field descriptors")
}

pub(super) fn method_ref(dex: &DexFile, method: MethodIdx) -> MethodRef {
    dex.method_descriptor(method)
        .parse()
        .expect("the DEX writes valid method descriptors")
}

/// What an instruction does with the item it references, by opcode.
pub(super) fn ref_kind(opcode: u16) -> Option<RefKind> {
    Some(match opcode {
        0x52..=0x58 | 0x60..=0x66 => RefKind::Get,
        0x59..=0x5f | 0x67..=0x6d => RefKind::Put,
        0x6e..=0x72 | 0x74..=0x78 | 0xfa..=0xfd => RefKind::Call,
        0x22 => RefKind::NewInstance,
        0x23..=0x25 => RefKind::NewArray,
        0x1f => RefKind::CheckCast,
        0x20 => RefKind::InstanceOf,
        0x1c => RefKind::ConstClass,
        0x1a | 0x1b => RefKind::LoadString,
        0x12..=0x19 | 0xd0..=0xe2 => RefKind::LoadNumber,
        _ => return None,
    })
}
