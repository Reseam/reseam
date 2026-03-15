use super::pattern::matches_pattern;
use super::{DexFile, InstructionPattern};
use crate::model::class::{ClassDef, EncodedMethod};
use crate::model::method::{MethodId, MethodIdx};
use crate::model::types::TypeIdx;

/// Borrowed search result pointing back into the owning `DexFile`.
#[derive(Debug)]
pub struct MethodMatch<'a> {
    pub class_idx: TypeIdx,
    pub method_idx: MethodIdx,
    pub class: &'a ClassDef,
    pub method: &'a EncodedMethod,
}

impl DexFile {
    /// Returns the first method satisfying the supplied predicate.
    pub fn find_method_by<F>(&self, predicate: F) -> Option<MethodMatch<'_>>
    where
        F: Fn(&MethodId, &ClassDef, &EncodedMethod) -> bool,
    {
        for class in &self.classes {
            if let Some(data) = class.class_data.as_ref() {
                for method in data.direct_methods.iter().chain(&data.virtual_methods) {
                    let method_id = &self.methods[method.method.0 as usize];
                    if predicate(method_id, class, method) {
                        return Some(MethodMatch {
                            class_idx: class.class_type,
                            method_idx: method.method,
                            class,
                            method,
                        });
                    }
                }
            }
        }
        None
    }

    /// Returns every method satisfying the supplied predicate.
    pub fn find_methods_by<F>(&self, predicate: F) -> Vec<MethodMatch<'_>>
    where
        F: Fn(&MethodId, &ClassDef, &EncodedMethod) -> bool,
    {
        let mut results = Vec::new();

        for class in &self.classes {
            if let Some(data) = class.class_data.as_ref() {
                for method in data.direct_methods.iter().chain(&data.virtual_methods) {
                    let method_id = &self.methods[method.method.0 as usize];
                    if predicate(method_id, class, method) {
                        results.push(MethodMatch {
                            class_idx: class.class_type,
                            method_idx: method.method,
                            class,
                            method,
                        });
                    }
                }
            }
        }

        results
    }

    /// Returns methods whose instruction stream contains the given opcode pattern.
    ///
    /// # Examples
    ///
    /// ```
    /// use stitch_dex::{DexFile, DexHeader, DexVersion, InstructionPattern, OpcodeMatcher};
    ///
    /// let dex = DexFile::new(DexHeader {
    ///     version: DexVersion::V035,
    ///     checksum: 0,
    ///     signature: [0; 20],
    ///     file_size: 0,
    ///     link_size: 0,
    ///     link_off: 0,
    ///     map_off: 0,
    ///     string_ids_size: 0,
    ///     string_ids_off: 0,
    ///     type_ids_size: 0,
    ///     type_ids_off: 0,
    ///     proto_ids_size: 0,
    ///     proto_ids_off: 0,
    ///     field_ids_size: 0,
    ///     field_ids_off: 0,
    ///     method_ids_size: 0,
    ///     method_ids_off: 0,
    ///     class_defs_size: 0,
    ///     class_defs_off: 0,
    ///     data_size: 0,
    ///     data_off: 0,
    /// });
    /// let pattern = [InstructionPattern::Opcode(OpcodeMatcher::ReturnVoid)];
    /// assert!(dex.find_methods_with_opcodes(&pattern).is_empty());
    /// ```
    pub fn find_methods_with_opcodes(
        &self,
        opcodes: &[InstructionPattern],
    ) -> Vec<MethodMatch<'_>> {
        let mut results = Vec::new();

        for class in &self.classes {
            if let Some(data) = class.class_data.as_ref() {
                for method in data.direct_methods.iter().chain(&data.virtual_methods) {
                    if let Some(code) = method.code.as_ref() {
                        if matches_pattern(&code.instructions, opcodes) {
                            results.push(MethodMatch {
                                class_idx: class.class_type,
                                method_idx: method.method,
                                class,
                                method,
                            });
                        }
                    }
                }
            }
        }

        results
    }
}
