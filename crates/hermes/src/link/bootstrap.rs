// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::Linker;
use crate::HermesFile;
use crate::assemble::encode;
use crate::edit::{EditedFunction, Edits, generated_function};
use crate::error::{HermesError, Result};
use crate::opcode::{Instruction, Op};
use crate::string::StringId;

impl Linker<'_> {
    /// Makes room for one more module in the bootstrap, appending the bootstrap function first.
    pub(super) fn add_bootstrap(&mut self) -> Result<()> {
        if self.edits.modules().len() >= usize::from(u16::MAX) {
            return Err(HermesError::Limit("65535 linked modules"));
        }
        if self.edits.bootstrap().is_none() {
            let name = self.string("reseamBootstrap")?;
            let id = self
                .edits
                .append_function(&self.file, generated_function(name, 1, 16, Vec::new()));
            self.edits.set_bootstrap(id);
        }
        Ok(())
    }
}

/// The function that initializes the linked modules into a private environment, in link order,
/// and then runs the app's global code. Its body is assembled here, once every module is known.
pub(crate) fn bootstrap(file: &HermesFile<'_>, edits: &Edits) -> Option<EditedFunction> {
    let bootstrap = edits.bootstrap()?;
    let modules = edits.modules();
    let mut code = vec![
        Instruction::new(Op::CreateTopLevelEnvironment, &[0, modules.len() as u64]),
        Instruction::new(Op::LoadConstUndefined, &[1]),
        Instruction::new(Op::LoadParam, &[2, 0]),
    ];
    let names = edits
        .link_names()
        .expect("linking a module names its interface");
    let string = |register: u64, id: StringId| {
        Instruction::new(Op::LoadConstStringLongIndex, &[register, u64::from(id.0)])
    };
    for (slot, module) in modules.iter().enumerate() {
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
            &[3, 0, u64::from(file.global_function().0)],
        ),
        Instruction::new(Op::Call1, &[3, 3, 2]),
        Instruction::new(Op::Ret, &[3]),
    ]);
    let name = StringId(
        edits
            .function(file, bootstrap)
            .expect("the bootstrap is appended")
            .header
            .name,
    );
    Some(generated_function(name, 1, 16, encode(&code)))
}
