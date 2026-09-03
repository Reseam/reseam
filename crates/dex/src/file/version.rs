// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::DexFile;
use crate::types::header::DexVersion;
use crate::types::instruction::Instruction;

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

fn uses_v038_instruction(instruction: &Instruction) -> bool {
    matches!(
        instruction,
        Instruction::InvokePolymorphic { .. }
            | Instruction::InvokePolymorphicRange { .. }
            | Instruction::InvokeCustom { .. }
            | Instruction::InvokeCustomRange { .. }
            | Instruction::ConstMethodHandle { .. }
            | Instruction::ConstMethodType { .. }
    )
}
