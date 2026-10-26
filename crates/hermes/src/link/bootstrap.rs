// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::assemble::encode;
use crate::edit::{EditedFunction, Editor, generated_function};
use crate::error::{Result, invalid};
use crate::opcode::{Instruction, Op};
use crate::string::{StringId, StringKind};

impl Editor<'_> {
    pub(crate) fn refresh_bootstrap(&mut self) -> Result<()> {
        if self.edits.modules.len() > u16::MAX as usize {
            return Err(invalid(0, "too many extension modules"));
        }
        if self.edits.global == self.file.global_function() {
            let name = self.intern("reseamBootstrap", StringKind::String)?;
            self.edits.global = self.append_function(generated_function(name, 1, 16, Vec::new()));
        }
        Ok(())
    }

    pub(crate) fn bootstrap(&self) -> Option<EditedFunction> {
        if self.edits.modules.is_empty() {
            return None;
        }
        let mut code = vec![
            Instruction::new(
                Op::CreateTopLevelEnvironment,
                &[0, self.edits.modules.len() as u64],
            ),
            Instruction::new(Op::LoadConstUndefined, &[1]),
            Instruction::new(Op::LoadParam, &[2, 0]),
        ];
        let names = self
            .edits
            .link_names
            .as_ref()
            .expect("linking a module names its interface");
        let string = |register: u64, id: StringId| {
            Instruction::new(Op::LoadConstStringLongIndex, &[register, u64::from(id.0)])
        };
        for (slot, module) in self.edits.modules.iter().enumerate() {
            code.extend([
                Instruction::new(
                    Op::CreateClosureLongIndex,
                    &[3, 1, u64::from(module.global.0)],
                ),
                Instruction::new(Op::Call1, &[3, 3, 1]),
                string(4, names.link),
                Instruction::new(Op::GetByVal, &[5, 3, 4]),
                Instruction::new(Op::NewObject, &[6]),
            ]);
            for import in &module.imports {
                code.extend([
                    Instruction::new(Op::NewObject, &[7]),
                    Instruction::new(Op::LoadConstInt, &[8, u64::from(import.id.0)]),
                    string(9, names.id),
                    Instruction::new(Op::DefineOwnByVal, &[7, 8, 9, 1]),
                    if import.es_module {
                        Instruction::new(Op::LoadConstTrue, &[8])
                    } else {
                        Instruction::new(Op::LoadConstFalse, &[8])
                    },
                    string(9, names.es_module),
                    Instruction::new(Op::DefineOwnByVal, &[7, 8, 9, 1]),
                    Instruction::new(Op::NewArray, &[8, 0]),
                ]);
                for (position, key) in import.keys.iter().enumerate() {
                    code.extend([
                        string(9, *key),
                        Instruction::new(Op::DefineOwnByIndexL, &[8, 9, position as u64]),
                    ]);
                }
                code.extend([
                    string(9, names.keys),
                    Instruction::new(Op::DefineOwnByVal, &[7, 8, 9, 1]),
                    string(9, import.specifier),
                    Instruction::new(Op::DefineOwnByVal, &[6, 7, 9, 1]),
                ]);
            }
            code.extend([
                Instruction::new(Op::Call2, &[3, 5, 3, 6]),
                Instruction::new(Op::StoreToEnvironmentL, &[0, slot as u64, 3]),
            ]);
        }
        code.extend([
            Instruction::new(
                Op::CreateClosureLongIndex,
                &[3, 0, u64::from(self.file.global_function().0)],
            ),
            Instruction::new(Op::Call1, &[3, 3, 2]),
            Instruction::new(Op::Ret, &[3]),
        ]);
        let name = StringId(
            self.edits.appended[(self.edits.global.0 - self.file.function_count()) as usize]
                .header
                .name,
        );
        Some(generated_function(name, 1, 16, encode(&code)))
    }
}
