use super::pattern::InstructionPattern;
use super::DexFile;
use crate::types::access_flags::AccessFlags;
use crate::types::class::{ClassDef, EncodedMethod};
use crate::types::instruction::Instruction;
use crate::types::{MethodIdx, StringIdx, TypeIdx};

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

#[derive(Debug)]
pub struct FingerprintMatch<'a> {
    pub class_idx: TypeIdx,
    pub method_idx: MethodIdx,
    pub class: &'a ClassDef,
    pub method: &'a EncodedMethod,
    pub matched_indices: Vec<u32>,
}

impl DexFile {
    pub fn find_method_by_fingerprint(&self, fp: &Fingerprint) -> Option<FingerprintMatch<'_>> {
        for class in &self.classes {
            if let Some(data) = class.class_data.as_ref() {
                for method in data.direct_methods.iter().chain(&data.virtual_methods) {
                    if let Some(m) = self.match_fingerprint(fp, class, method) {
                        return Some(m);
                    }
                }
            }
        }
        None
    }

    pub fn find_methods_by_fingerprint(&self, fp: &Fingerprint) -> Vec<FingerprintMatch<'_>> {
        let mut results = Vec::new();
        for class in &self.classes {
            if let Some(data) = class.class_data.as_ref() {
                for method in data.direct_methods.iter().chain(&data.virtual_methods) {
                    if let Some(m) = self.match_fingerprint(fp, class, method) {
                        results.push(m);
                    }
                }
            }
        }
        results
    }

    fn match_fingerprint<'a>(
        &'a self,
        fp: &Fingerprint,
        class: &'a ClassDef,
        method: &'a EncodedMethod,
    ) -> Option<FingerprintMatch<'a>> {
        let method_id = &self.methods[method.method.0 as usize];

        if let Some(ref defining_class) = fp.defining_class {
            let desc = self.type_descriptor(class.class_type);
            if desc != defining_class {
                return None;
            }
        }

        if let Some(ref name) = fp.name {
            let method_name = self.string(method_id.name);
            if method_name != name {
                return None;
            }
        }

        if let Some(ref flags) = fp.access_flags {
            if !method.access_flags.contains(*flags) {
                return None;
            }
        }

        let proto = &self.prototypes[method_id.proto.0 as usize];

        if let Some(ref return_type) = fp.return_type {
            let ret_desc = self.type_descriptor(proto.return_type);
            if !ret_desc.starts_with(return_type.as_str()) {
                return None;
            }
        }

        if let Some(ref parameters) = fp.parameters {
            if proto.parameters.len() != parameters.len() {
                return None;
            }
            for (param_idx, expected) in proto.parameters.iter().zip(parameters) {
                let param_desc = self.type_descriptor(*param_idx);
                if !param_matches(param_desc, expected) {
                    return None;
                }
            }
        }

        if let Some(ref strings) = fp.strings {
            let code = method.code.as_ref()?;
            let string_indices: Vec<StringIdx> = strings
                .iter()
                .filter_map(|s| self.find_string_idx(s))
                .collect();
            if string_indices.len() != strings.len() {
                return None;
            }
            for &target_idx in &string_indices {
                let found = code.instructions.iter().any(|insn| match insn {
                    Instruction::ConstString { string, .. }
                    | Instruction::ConstStringJumbo { string, .. } => *string == target_idx,
                    _ => false,
                });
                if !found {
                    return None;
                }
            }
        }

        if let Some(ref literals) = fp.literals {
            let code = method.code.as_ref()?;
            for &target in literals {
                let found = code
                    .instructions
                    .iter()
                    .any(|insn| insn.literal() == Some(target));
                if !found {
                    return None;
                }
            }
        }

        let matched_indices = if let Some(ref opcodes) = fp.opcodes {
            let code = method.code.as_ref()?;
            match find_pattern_indices(&code.instructions, opcodes) {
                Some(indices) => indices,
                None => return None,
            }
        } else {
            Vec::new()
        };

        Some(FingerprintMatch {
            class_idx: class.class_type,
            method_idx: method.method,
            class,
            method,
            matched_indices,
        })
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

fn find_pattern_indices(
    instructions: &[Instruction],
    pattern: &[InstructionPattern],
) -> Option<Vec<u32>> {
    if pattern.is_empty() {
        return Some(Vec::new());
    }
    if instructions.len() < pattern.len() {
        return None;
    }
    'outer: for start in 0..=instructions.len() - pattern.len() {
        for (i, pat) in pattern.iter().enumerate() {
            match pat {
                InstructionPattern::Any => {}
                InstructionPattern::Opcode(matcher) => {
                    if !matcher.matches(&instructions[start + i]) {
                        continue 'outer;
                    }
                }
                InstructionPattern::OpcodeValue(op) => {
                    if instructions[start + i].opcode() != Some(*op) {
                        continue 'outer;
                    }
                }
            }
        }
        let indices: Vec<u32> = (start..start + pattern.len()).map(|i| i as u32).collect();
        return Some(indices);
    }
    None
}
