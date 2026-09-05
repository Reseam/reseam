// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::pattern::{find_pattern_span, InstructionPattern};
use super::scan::{MethodHit, MethodView};
use super::{DexFile, RefKey, RefQuery};
use crate::error::Result;
use crate::types::access_flags::AccessFlags;
use crate::types::{StringIdx, TypeIdx};

#[derive(Debug, Clone)]
pub struct Fingerprint {
    pub access_flags: Option<AccessFlags>,
    pub return_type: Option<String>,
    pub parameters: Option<Vec<String>>,
    pub strings: Option<Vec<String>>,
    pub literals: Option<Vec<i64>>,
    pub defining_class: Option<String>,
    pub name: Option<String>,
    pub opcodes: Option<Vec<InstructionPattern>>,
}

impl Fingerprint {
    pub fn builder() -> FingerprintBuilder {
        FingerprintBuilder {
            inner: Fingerprint {
                access_flags: None,
                return_type: None,
                parameters: None,
                strings: None,
                literals: None,
                defining_class: None,
                name: None,
                opcodes: None,
            },
        }
    }
}

pub struct FingerprintBuilder {
    inner: Fingerprint,
}

impl FingerprintBuilder {
    pub fn access_flags(mut self, flags: AccessFlags) -> Self {
        self.inner.access_flags = Some(flags);
        self
    }

    pub fn return_type(mut self, ret: impl Into<String>) -> Self {
        self.inner.return_type = Some(ret.into());
        self
    }

    pub fn parameters(mut self, params: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.inner.parameters = Some(params.into_iter().map(Into::into).collect());
        self
    }

    pub fn strings(mut self, strings: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.inner.strings = Some(strings.into_iter().map(Into::into).collect());
        self
    }

    pub fn defining_class(mut self, class: impl Into<String>) -> Self {
        self.inner.defining_class = Some(class.into());
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.inner.name = Some(name.into());
        self
    }

    pub fn literals(mut self, literals: impl IntoIterator<Item = i64>) -> Self {
        self.inner.literals = Some(literals.into_iter().collect());
        self
    }

    pub fn opcodes(mut self, opcodes: impl IntoIterator<Item = InstructionPattern>) -> Self {
        self.inner.opcodes = Some(opcodes.into_iter().collect());
        self
    }

    pub fn build(self) -> Fingerprint {
        self.inner
    }
}

#[derive(Debug, Clone)]
pub struct FingerprintHit {
    pub method: MethodHit,
    pub matched_indices: Vec<u32>,
}

/// A fingerprint with its required strings resolved to DEX indices once. `None`
/// from [`DexFile::prepare_fingerprint`] means a required string is absent from
/// the DEX, so no method can match and scanning is skipped entirely.
struct PreparedFingerprint {
    defining_class: Option<TypeIdx>,
    name: Option<StringIdx>,
    strings: Option<Vec<StringIdx>>,
}

impl PreparedFingerprint {
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

    /// Resolves the fingerprint's names to this DEX's indices once; a DEX that
    /// lacks any of them cannot contain a match and is skipped entirely.
    fn prepare_fingerprint(&self, fp: &Fingerprint) -> Option<PreparedFingerprint> {
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
        })
    }

    /// Checks a fingerprint against one method. Metadata criteria (defining
    /// class, name, flags, prototype) are all id-table reads and run first;
    /// instructions decode only if those pass and the fingerprint needs them.
    fn match_fingerprint(
        &self,
        fp: &Fingerprint,
        prepared: &PreparedFingerprint,
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

        if let Some(ref flags) = fp.access_flags {
            if !view.access_flags.contains(*flags) {
                return Ok(None);
            }
        }

        let proto = self.proto(method_id.proto);

        if let Some(ref return_type) = fp.return_type {
            if !self
                .type_descriptor(proto.return_type)
                .starts_with(return_type.as_str())
            {
                return Ok(None);
            }
        }

        if let Some(ref parameters) = fp.parameters {
            if proto.parameters.len() != parameters.len() {
                return Ok(None);
            }
            for (param_idx, expected) in proto.parameters.iter().zip(parameters) {
                if !param_matches(&self.type_descriptor(*param_idx), expected) {
                    return Ok(None);
                }
            }
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

fn param_matches(actual: &str, expected: &str) -> bool {
    if actual == expected {
        return true;
    }
    // "L" matches any object type (Lcom/foo/Bar;), "[" matches any array type.
    if expected == "L" {
        return actual.starts_with('L');
    }
    if expected == "[" {
        return actual.starts_with('[');
    }
    false
}
