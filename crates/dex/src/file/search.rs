// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::pattern::matches_pattern;
use super::{DexFile, InstructionPattern};
use crate::types::class::{ClassDef, EncodedMethod};
use crate::types::{MethodId, MethodIdx, TypeIdx};

#[derive(Debug)]
pub struct MethodMatch<'a> {
    pub class_idx: TypeIdx,
    pub method_idx: MethodIdx,
    pub class: &'a ClassDef,
    pub method: &'a EncodedMethod,
}

impl DexFile {
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
