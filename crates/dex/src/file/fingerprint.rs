// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::pattern::{InstructionPattern, find_pattern_span};
use super::scan::{MethodHit, MethodView};
use super::{DexFile, RefKey, RefQuery};
use crate::error::Result;
use crate::types::access_flags::AccessFlags;
use crate::types::{StringIdx, TypeIdx};

#[derive(Debug, Clone, Default)]
pub struct Fingerprint {
    pub access_flags: Option<AccessFlags>,
    pub return_type: Option<TypePattern>,
    pub parameters: Option<Vec<TypePattern>>,
    pub strings: Option<Vec<String>>,
    pub literals: Option<Vec<i64>>,
    pub defining_class: Option<String>,
    pub name: Option<String>,
    pub opcodes: Option<Vec<InstructionPattern>>,
}

#[derive(Debug, Clone)]
pub struct FingerprintHit {
    pub method: MethodHit,
    pub matched_indices: Vec<u32>,
}

struct PreparedFingerprint<'a> {
    defining_class: Option<TypeIdx>,
    name: Option<StringIdx>,
    strings: Option<Vec<StringIdx>>,
    return_type: Option<&'a TypePattern>,
    parameters: Option<&'a [TypePattern]>,
}

impl PreparedFingerprint<'_> {
    fn query(&self, fp: &Fingerprint) -> RefQuery {
        let strings = self.strings.iter().flatten().map(|&s| RefKey::string(s));
        let literals = fp.literals.iter().flatten().map(|&l| RefKey::literal(l));
        RefQuery::all_of(strings.chain(literals))
    }
}

impl DexFile {
    pub fn find_method_by_fingerprint(&self, fp: &Fingerprint) -> Result<Option<FingerprintHit>> {
        let Some(prepared) = self.prepare_fingerprint(fp) else {
            return Ok(None);
        };
        self.scan_methods_find(&prepared.query(fp), |view| {
            self.match_fingerprint(fp, &prepared, view)
        })
    }

    pub fn find_methods_by_fingerprint(&self, fp: &Fingerprint) -> Result<Vec<FingerprintHit>> {
        let Some(prepared) = self.prepare_fingerprint(fp) else {
            return Ok(Vec::new());
        };
        self.scan_methods_collect(&prepared.query(fp), |view| {
            self.match_fingerprint(fp, &prepared, view)
        })
    }

    fn prepare_fingerprint<'a>(&self, fp: &'a Fingerprint) -> Option<PreparedFingerprint<'a>> {
        let defining_class = match &fp.defining_class {
            None => None,
            Some(descriptor) => Some(self.find_type_idx(descriptor)?),
        };
        let name = match &fp.name {
            None => None,
            Some(name) => Some(self.find_string_idx(name)?),
        };
        let strings = match &fp.strings {
            None => None,
            Some(list) => Some(
                list.iter()
                    .map(|s| self.find_string_idx(s))
                    .collect::<Option<_>>()?,
            ),
        };
        Some(PreparedFingerprint {
            defining_class,
            name,
            strings,
            return_type: fp.return_type.as_ref(),
            parameters: fp.parameters.as_deref(),
        })
    }

    fn match_fingerprint(
        &self,
        fp: &Fingerprint,
        prepared: &PreparedFingerprint<'_>,
        view: &MethodView<'_>,
    ) -> Result<Option<FingerprintHit>> {
        let method_id = self.method_id(view.method);

        if prepared
            .defining_class
            .is_some_and(|t| t != view.class_type)
        {
            return Ok(None);
        }

        if prepared.name.is_some_and(|n| n != method_id.name) {
            return Ok(None);
        }

        if let Some(ref flags) = fp.access_flags
            && !view.access_flags.contains(*flags)
        {
            return Ok(None);
        }

        let proto = self.proto(method_id.proto);

        if prepared
            .return_type
            .as_ref()
            .is_some_and(|criterion| !criterion.matches(&self.type_descriptor(proto.return_type)))
        {
            return Ok(None);
        }
        if let Some(parameters) = prepared.parameters
            && (proto.parameters.len() != parameters.len()
                || !proto
                    .parameters
                    .iter()
                    .zip(parameters)
                    .all(|(actual, criterion)| criterion.matches(&self.type_descriptor(*actual))))
        {
            return Ok(None);
        }

        let needs_instructions =
            prepared.strings.is_some() || fp.literals.is_some() || fp.opcodes.is_some();
        if !needs_instructions {
            return Ok(Some(FingerprintHit {
                method: view.hit(),
                matched_indices: Vec::new(),
            }));
        }
        if !view.has_code() {
            return Ok(None);
        }

        for &target in prepared.strings.iter().flatten() {
            if !view.any_instruction(|insn| insn.string_ref() == Some(target))? {
                return Ok(None);
            }
        }

        for &target in fp.literals.iter().flatten() {
            if !view.any_instruction(|insn| insn.literal() == Some(target))? {
                return Ok(None);
            }
        }

        let matched_indices = if let Some(ref opcodes) = fp.opcodes {
            let mut seq = Vec::new();
            view.opcodes(&mut seq)?;
            match find_pattern_span(&seq, opcodes) {
                Some(span) => span.map(|index| index as u32).collect(),
                None => return Ok(None),
            }
        } else {
            Vec::new()
        };

        Ok(Some(FingerprintHit {
            method: view.hit(),
            matched_indices,
        }))
    }
}

/// A descriptor match policy. Exact and prefix matching operate on DEX descriptor text;
/// Object matches class references and Array matches all array dimensions and element types.
#[derive(Debug, Clone)]
pub enum TypePattern {
    Exact(String),
    Prefix(String),
    Object,
    Array,
}

impl TypePattern {
    fn matches(&self, descriptor: &str) -> bool {
        match self {
            Self::Exact(value) => descriptor == value,
            Self::Prefix(value) => descriptor.starts_with(value),
            Self::Object => descriptor.starts_with('L'),
            Self::Array => descriptor.starts_with('['),
        }
    }
}
