// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::assemble::assemble;
use crate::edit::{Edits, LinkNames, LinkedImport, LinkedModule, ModuleId};
use crate::error::{HermesError, LinkError, Result};
use crate::layout::{Field, STATIC_BUILTINS, Section};
use crate::metro::MetroModules;
use crate::opcode::Op;
use crate::string::{StringId, StringKind};
use crate::{FunctionId, FunctionIndex, HermesFile};

mod bootstrap;
mod literals;
mod remap;
#[cfg(test)]
mod tests;

pub(crate) use bootstrap::bootstrap;
use literals::Literals;
use remap::{Moved, Remap};

/// An app module a linked module imports: its specifier, and an app function the Metro module's
/// factory creates, or the factory itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    pub specifier: String,
    pub function: FunctionId,
}

/// Links modules into one file: its edits, and what the app's Metro modules export.
pub(crate) struct Linker<'e> {
    pub file: HermesFile<'e>,
    pub index: &'e FunctionIndex,
    pub metro: &'e mut MetroModules,
    pub edits: &'e mut Edits,
}

impl Linker<'_> {
    pub(crate) fn link(&mut self, module: &HermesFile<'_>, imports: &[Import]) -> Result<ModuleId> {
        standalone(&self.file, module)?;
        let interface = crate::interface::interface(module)?;
        let imports = self.imports(&interface.imports, imports)?;
        let strings = module
            .strings()
            .map(|(id, kind)| Ok(self.edits.strings_mut().append(module.string(id)?, kind)?.0))
            .collect::<Result<Vec<_>>>()?;
        let moved = |count: Field, section: Section| Moved {
            base: self.edits.section_count(&self.file, section),
            count: module.field(count),
        };
        let mut remap = Remap {
            strings: &strings,
            functions: Moved {
                base: self.edits.function_count(&self.file),
                count: module.function_count(),
            },
            bigints: moved(Field::BigIntCount, Section::BigInts),
            regexps: moved(Field::RegExpCount, Section::Regexps),
            shapes: moved(Field::ObjectShapeCount, Section::Shapes),
            switches: Moved {
                base: self.edits.string_switches(),
                count: module.field(Field::StringSwitchCount),
            },
            values: Literals::new(
                module,
                Section::Values,
                &strings,
                self.edits.section_size(&self.file, Section::Values) as u32,
            ),
            shape_table: module.section(Section::Shapes).as_chunks::<8>().0,
        };
        literals::copy_shapes(&self.file, self.edits, module, &strings)?;
        literals::copy_storage(&self.file, self.edits, module)?;
        self.functions(module, &mut remap)?;
        self.edits.append(Section::Values, &remap.values.output);
        self.edits.add_string_switches(remap.switches.count)?;
        let names = LinkNames {
            link: self.string("link")?,
            id: self.string("id")?,
            es_module: self.string("esModule")?,
            keys: self.string("keys")?,
        };
        self.add_bootstrap()?;
        let module = LinkedModule {
            global: FunctionId(remap.functions.base + module.global_function().0),
            imports,
            exports: interface
                .exports
                .into_iter()
                .map(|id| StringId(strings[id.0 as usize]))
                .collect(),
        };
        Ok(self.edits.add_module(module, names))
    }

    /// Resolves each import the module declares to the Metro module it names.
    fn imports(&mut self, declared: &[String], imports: &[Import]) -> Result<Vec<LinkedImport>> {
        if let Some(missing) = declared
            .iter()
            .find(|specifier| imports.iter().all(|import| &import.specifier != *specifier))
        {
            return Err(HermesError::Link(LinkError::Unresolved(missing.clone())));
        }
        let mut resolved = Vec::with_capacity(imports.len());
        for (position, import) in imports.iter().enumerate() {
            if !declared.contains(&import.specifier)
                || imports[..position]
                    .iter()
                    .any(|other| other.specifier == import.specifier)
            {
                return Err(HermesError::Link(LinkError::Unimported(
                    import.specifier.clone(),
                )));
            }
            let (id, shape) = self.metro.module(&self.file, self.index, import.function)?;
            resolved.push(LinkedImport {
                specifier: self.string(&import.specifier)?,
                id,
                es_module: shape.es_module,
                keys: shape.keys.into_iter().collect(),
            });
        }
        Ok(resolved)
    }

    /// Appends the module's functions with every reference moved.
    fn functions(&mut self, module: &HermesFile<'_>, remap: &mut Remap<'_>) -> Result<()> {
        for id in 0..module.function_count() {
            let function = module.function(FunctionId(id))?;
            let mut instructions = function.instructions()?;
            for inst in &mut instructions {
                remap.instruction(inst, function.header.offset as usize)?;
            }
            let mut edited = assemble(&function, instructions, Some(remap.strings))?;
            edited.header.name = remap.strings[function.header.name as usize];
            self.edits.append_function(&self.file, edited);
        }
        Ok(())
    }

    fn string(&mut self, text: &str) -> Result<StringId> {
        self.edits
            .strings_mut()
            .intern(&self.file, text, StringKind::String)
    }
}

/// Fails unless `module` is standalone execution bytecode that keeps its declarations to itself
/// and assumes no static builtins the app does not.
fn standalone(app: &HermesFile<'_>, module: &HermesFile<'_>) -> Result<()> {
    if module.field(Field::CjsModuleCount) != 0 || module.field(Field::SegmentId) != 0 {
        return Err(HermesError::Link(LinkError::Segmented));
    }
    if module.options() & STATIC_BUILTINS != 0 && app.options() & STATIC_BUILTINS == 0 {
        return Err(HermesError::Link(LinkError::StaticBuiltins));
    }
    if module
        .function(module.global_function())?
        .instructions()?
        .iter()
        .any(|i| i.op == Op::DeclareGlobalVar)
    {
        return Err(HermesError::Link(LinkError::Globals));
    }
    Ok(())
}
