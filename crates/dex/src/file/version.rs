// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::DexFile;
use crate::instruction::Instruction;
use crate::instruction::opcodes::{CONST_METHOD_TYPE, INVOKE_POLYMORPHIC};
use crate::model::header::DexVersion;

impl DexFile {
    pub fn version(&self) -> DexVersion {
        self.header.version
    }

    /// The version the file must be written as: never below what it was
    /// parsed as (classes still in the file keep whatever they use), raised
    /// by anything a patch added.
    pub fn required_version(&self) -> DexVersion {
        let mut version = self.header.version;
        if version >= DexVersion::V040 {
            return version;
        }
        if self.hidden_api.is_some() {
            version = version.max(DexVersion::V039);
        }
        if !self.call_sites.is_empty() || !self.method_handles.is_empty() {
            version = version.max(DexVersion::V038);
        }
        if version < DexVersion::V038 && self.resident_uses_v038_instruction() {
            version = DexVersion::V038;
        }
        version
    }

    fn resident_uses_v038_instruction(&self) -> bool {
        (0..self.classes.len())
            .filter_map(|i| self.classes.resident(i))
            .filter_map(|class| class.class_data.as_deref())
            .flat_map(|data| data.direct_methods.iter().chain(&data.virtual_methods))
            .filter_map(|method| method.code.as_ref())
            .any(|code| code.instructions.iter().any(uses_v038_instruction))
    }
}

/// The opcodes from `invoke-polymorphic` on came with DEX 038.
fn uses_v038_instruction(instruction: &Instruction) -> bool {
    instruction
        .opcode()
        .is_some_and(|opcode| (INVOKE_POLYMORPHIC..=CONST_METHOD_TYPE).contains(&opcode))
}
