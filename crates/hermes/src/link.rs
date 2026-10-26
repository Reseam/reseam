// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::assemble::assemble;
use crate::bytes::read_u32;
use crate::edit::Editor;
use crate::error::{HermesError, Result, invalid};
use crate::layout::{Field, STATIC_BUILTINS, Section};
use crate::metro::MetroModuleId;
use crate::opcode::{IdKind, Op};
use crate::string::{StringId, StringKind};
use crate::{FunctionId, FunctionIndex, HermesFile};

mod bootstrap;
mod literals;
#[cfg(test)]
mod tests;

pub(crate) use bootstrap::generated_function;
use literals::{Literals, at, checked_id, literal_count};

/// Identity of a module linked into this editor. Its exports remain in a
/// private environment and are initialized before the app's global code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleId(pub(crate) u32);

/// An app module a linked module imports: its specifier, and an app function the Metro module's
/// factory creates, or the factory itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    pub specifier: String,
    pub function: FunctionId,
}

/// A linked module: its global function, what each import resolves to, and its exports.
pub(crate) struct LinkedModule {
    pub global: FunctionId,
    pub imports: Vec<LinkedImport>,
    pub exports: Vec<StringId>,
}

/// An import as `link` receives it: `{ id, esModule, keys }` under the specifier, enough to read
/// the Metro module lazily, the first time the module's exports are read.
pub(crate) struct LinkedImport {
    pub specifier: StringId,
    pub id: MetroModuleId,
    pub es_module: bool,
    pub keys: Vec<StringId>,
}

/// The property names of a linked module's interface and of each import record.
#[derive(Clone, Copy)]
pub(crate) struct LinkNames {
    pub link: StringId,
    pub id: StringId,
    pub es_module: StringId,
    pub keys: StringId,
}

/// Where a linked module's IDs start in the merged file, by kind.
struct Bases {
    function: u32,
    bigint: u32,
    regexp: u32,
    shape: u32,
    switch: u32,
}

impl Editor<'_> {
    /// Links a compiled v98 JavaScript module and returns its stable identity.
    /// The module evaluates to `{ imports, link }` and keeps declarations in its
    /// own lexical scope (normally an IIFE); see [`imports`](crate::imports).
    /// `imports` resolves each declared specifier, exactly once. Before the
    /// app's global code runs, `link` is called with an object mapping each
    /// specifier to its Metro module ID, and returns the exports object; its
    /// exports are the keys every return path of `link` declares. CJS segments,
    /// global variable declarations and incompatible static-builtin
    /// assumptions are refused. Every function, string, bigint, regexp, literal
    /// and shape reference is remapped. Module debug information is dropped;
    /// exceptions are retained. The shared module initializer is assembled
    /// once when the file is written.
    pub fn link(
        &mut self,
        index: &FunctionIndex,
        module: &HermesFile<'_>,
        imports: &[Import],
    ) -> Result<ModuleId> {
        index.check(&self.file)?;
        self.transaction(|editor| editor.link_module(index, module, imports))
    }

    #[expect(
        clippy::too_many_lines,
        reason = "linking visits each versioned file segment in order"
    )]
    fn link_module(
        &mut self,
        index: &FunctionIndex,
        module: &HermesFile<'_>,
        imports: &[Import],
    ) -> Result<ModuleId> {
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
            let (id, shape) = self.metro_shape(index, import.function)?;
            resolved.push(LinkedImport {
                specifier: self.intern(&import.specifier, StringKind::String)?,
                id,
                es_module: shape.es_module,
                keys: shape.keys.into_iter().collect(),
            });
        }
        let mut strings = Vec::with_capacity(module.string_count() as usize);
        for (id, kind) in module.strings() {
            let value = module.string(id)?;
            let length = value.bytes.len() / value.encoding.unit_size();
            strings.push(
                self.append_string(value.bytes.to_vec(), value.encoding, length as u32, kind)?
                    .0,
            );
        }
        let bases = Bases {
            function: self.next_function().0,
            bigint: self.section_count(Section::BigInts),
            regexp: self.section_count(Section::Regexps),
            shape: self.section_count(Section::Shapes),
            switch: self.edits.string_switches,
        };
        let shapes = module.section(Section::Shapes).as_chunks::<8>().0;
        let mut values = Literals::new(
            module.section(Section::Values),
            &strings,
            self.section_size(Section::Values) as u32,
        );
        let mut keys = Literals::new(
            module.section(Section::Keys),
            &strings,
            self.section_size(Section::Keys) as u32,
        );
        for entry in shapes {
            let offset = keys.relocate(read_u32(entry, 0)?, read_u32(entry, 4)?)?;
            self.edits.additions[Section::Shapes as usize].extend_from_slice(&offset.to_le_bytes());
            self.edits.additions[Section::Shapes as usize].extend_from_slice(&entry[4..]);
        }
        for (table, storage) in [
            (Section::BigInts, Section::BigIntStorage),
            (Section::Regexps, Section::RegexpStorage),
        ] {
            let base = self.section_size(storage) as u32;
            for entry in module.section(table).as_chunks::<8>().0 {
                let offset = read_u32(entry, 0)?
                    .checked_add(base)
                    .ok_or_else(|| invalid(0, "literal storage offset overflow"))?;
                self.edits.additions[table as usize].extend_from_slice(&offset.to_le_bytes());
                self.edits.additions[table as usize].extend_from_slice(&entry[4..]);
            }
            self.edits.additions[storage as usize].extend_from_slice(module.section(storage));
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
            self.append_function(edited);
        }
        self.edits.additions[Section::Values as usize].extend(values.output);
        self.edits.additions[Section::Keys as usize].extend(keys.output);
        self.edits.string_switches = self
            .edits
            .string_switches
            .checked_add(module.field(Field::StringSwitchCount))
            .ok_or_else(|| invalid(0, "too many string switches"))?;
        let module_id = ModuleId(self.edits.modules.len() as u32);
        self.edits.link_names = Some(LinkNames {
            link: self.intern("link", StringKind::String)?,
            id: self.intern("id", StringKind::String)?,
            es_module: self.intern("esModule", StringKind::String)?,
            keys: self.intern("keys", StringKind::String)?,
        });
        self.edits.modules.push(LinkedModule {
            global: FunctionId(bases.function + module.global_function().0),
            imports: resolved,
            exports: interface
                .exports
                .into_iter()
                .map(|id| StringId(strings[id.0 as usize]))
                .collect(),
        });
        self.refresh_bootstrap()?;
        Ok(module_id)
    }
}
