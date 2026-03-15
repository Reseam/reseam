use super::DexFile;
use crate::model::header::DexVersion;
use crate::model::instruction::Instruction;

impl DexFile {
    /// Returns the header-declared DEX version.
    pub fn version(&self) -> DexVersion {
        self.header.version
    }

    /// Computes the minimum DEX version required to encode the current contents.
    pub fn required_version(&self) -> DexVersion {
        if self.hidden_api.is_some() {
            return DexVersion::V039;
        }
        if !self.call_sites.is_empty() || !self.method_handles.is_empty() {
            return DexVersion::V038;
        }

        for class in &self.classes {
            if let Some(data) = class.class_data.as_ref() {
                for method in data.direct_methods.iter().chain(&data.virtual_methods) {
                    if let Some(code) = method.code.as_ref() {
                        if code.instructions.iter().any(uses_v038_instruction) {
                            return DexVersion::V038;
                        }
                    }
                }
            }
        }

        DexVersion::V035
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
