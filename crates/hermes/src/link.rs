// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::assemble::assemble;
use crate::bytes::read_u32;
use crate::edit::{Edits, LinkNames, LinkedImport, LinkedModule, ModuleId};
use crate::error::{HermesError, Result, invalid};
use crate::layout::{Field, STATIC_BUILTINS, Section};
use crate::metro::MetroModules;
use crate::opcode::{IdKind, Op};
use crate::string::{StringId, StringKind};
use crate::{FunctionId, FunctionIndex, HermesFile};

mod bootstrap;
mod literals;
#[cfg(test)]
mod tests;

pub(crate) use bootstrap::bootstrap;
use literals::{Literals, at, checked_id, literal_count};

/// An app module a linked module imports: its specifier, and an app function the Metro module's
/// factory creates, or the factory itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    pub specifier: String,
    pub function: FunctionId,
}

/// Where a linked module's IDs start in the merged file, by kind.
struct Bases {
    function: u32,
    bigint: u32,
    regexp: u32,
    shape: u32,
    switch: u32,
}

/// Links modules into one file: its edits, and what the app's Metro modules export.
pub(crate) struct Linker<'e> {
    pub file: HermesFile<'e>,
    pub index: &'e FunctionIndex,
    pub metro: &'e mut MetroModules,
    pub edits: &'e mut Edits,
}

impl Linker<'_> {
    #[expect(
        clippy::too_many_lines,
        reason = "linking visits each file segment in order"
    )]
    pub(crate) fn link(&mut self, module: &HermesFile<'_>, imports: &[Import]) -> Result<ModuleId> {
        if module.field(Field::CjsModuleCount) != 0 || module.field(Field::SegmentId) != 0 {
            return Err(HermesError::Unsupported(
                "extensions must be standalone execution bytecode; CommonJS segments are unsupported".into(),
            ));
        }
        if module.options() & STATIC_BUILTINS != 0 && self.file.options() & STATIC_BUILTINS == 0 {
            return Err(HermesError::Unsupported(
                "extension assumes static builtins but app does not".into(),
            ));
        }
        if module
            .function(module.global_function())?
            .instructions()?
            .iter()
            .any(|i| i.op == Op::DeclareGlobalVar)
        {
            return Err(HermesError::Unsupported(
                "extension declares globals; compile an IIFE returning exports".into(),
            ));
        }
        let interface = crate::interface::interface(module)?;
        if let Some(missing) = interface
            .imports
            .iter()
            .find(|specifier| imports.iter().all(|import| &import.specifier != *specifier))
        {
            return Err(HermesError::Unsupported(format!(
                "module imports {missing:?}, which is not resolved"
            )));
        }
        let mut resolved = Vec::with_capacity(imports.len());
        for (position, import) in imports.iter().enumerate() {
            if !interface.imports.contains(&import.specifier)
                || imports[..position]
                    .iter()
                    .any(|other| other.specifier == import.specifier)
            {
                return Err(HermesError::Unsupported(format!(
                    "{:?} is resolved but not imported once by the module",
                    import.specifier
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
        let mut strings = Vec::with_capacity(module.string_count() as usize);
        for (id, kind) in module.strings() {
            strings.push(self.edits.strings_mut().append(module.string(id)?, kind)?.0);
        }
        let bases = Bases {
            function: self.edits.function_count(&self.file),
            bigint: self.edits.section_count(&self.file, Section::BigInts),
            regexp: self.edits.section_count(&self.file, Section::Regexps),
            shape: self.edits.section_count(&self.file, Section::Shapes),
            switch: self.edits.string_switches(),
        };
        let shapes = module.section(Section::Shapes).as_chunks::<8>().0;
        let mut values = Literals::new(
            module.section(Section::Values),
            &strings,
            self.edits.section_size(&self.file, Section::Values) as u32,
        );
        let mut keys = Literals::new(
            module.section(Section::Keys),
            &strings,
            self.edits.section_size(&self.file, Section::Keys) as u32,
        );
        for entry in shapes {
            let offset = keys.relocate(read_u32(entry, 0)?, read_u32(entry, 4)?)?;
            self.edits.append(Section::Shapes, &offset.to_le_bytes());
            self.edits.append(Section::Shapes, &entry[4..]);
        }
        for (table, storage) in [
            (Section::BigInts, Section::BigIntStorage),
            (Section::Regexps, Section::RegexpStorage),
        ] {
            let base = self.edits.section_size(&self.file, storage) as u32;
            for entry in module.section(table).as_chunks::<8>().0 {
                let offset = read_u32(entry, 0)?
                    .checked_add(base)
                    .ok_or_else(|| invalid(0, "literal storage offset overflow"))?;
                self.edits.append(table, &offset.to_le_bytes());
                self.edits.append(table, &entry[4..]);
            }
            self.edits.append(storage, module.section(storage));
        }
        for id in 0..module.function_count() {
            let function = module.function(FunctionId(id))?;
            let mut instructions = function.instructions()?;
            for inst in &mut instructions {
                let at = at(inst);
                let read_values = literal_count(inst, shapes)?;
                for (operand, value) in inst.definition().operands.iter().zip(&mut inst.values) {
                    *value = u64::from(match operand.id {
                        IdKind::None => continue,
                        IdKind::String => *strings
                            .get(*value as usize)
                            .ok_or_else(|| invalid(at, "string ID out of range"))?,
                        IdKind::Function => {
                            checked_id(*value, module.function_count(), bases.function)?
                        }
                        IdKind::BigInt => {
                            checked_id(*value, module.field(Field::BigIntCount), bases.bigint)?
                        }
                        IdKind::RegExp => {
                            checked_id(*value, module.field(Field::RegExpCount), bases.regexp)?
                        }
                        IdKind::Shape => {
                            checked_id(*value, module.field(Field::ObjectShapeCount), bases.shape)?
                        }
                        IdKind::Switch => checked_id(
                            *value,
                            module.field(Field::StringSwitchCount),
                            bases.switch,
                        )?,
                        IdKind::ValueBuffer => values.relocate(
                            *value as u32,
                            read_values
                                .ok_or_else(|| invalid(at, "value buffer without a length"))?,
                        )?,
                    });
                }
            }
            let mut edited = assemble(&function, instructions, Some(&strings))?;
            edited.header.name = strings[function.header.name as usize];
            self.edits.append_function(&self.file, edited);
        }
        self.edits.append(Section::Values, &values.output);
        self.edits.append(Section::Keys, &keys.output);
        self.edits
            .add_string_switches(module.field(Field::StringSwitchCount))?;
        let names = LinkNames {
            link: self.string("link")?,
            id: self.string("id")?,
            es_module: self.string("esModule")?,
            keys: self.string("keys")?,
        };
        self.add_bootstrap()?;
        Ok(self.edits.add_module(
            LinkedModule {
                global: FunctionId(bases.function + module.global_function().0),
                imports: resolved,
                exports: interface
                    .exports
                    .into_iter()
                    .map(|id| StringId(strings[id.0 as usize]))
                    .collect(),
            },
            names,
        ))
    }

    fn string(&mut self, text: &str) -> Result<StringId> {
        self.edits
            .strings_mut()
            .intern(&self.file, text, StringKind::String)
    }
}
