use std::collections::BTreeMap;

use crate::assemble::{assemble, encode, instruction};
use crate::edit::{EditedFunction, Editor, FunctionBody};
use crate::error::{HermesError, Result, invalid};
use crate::model::{FunctionHeader, Section};
use crate::opcode::IdKind;
use crate::parse::{read_u32, slice};
use crate::{FunctionId, HermesFile, StringId, StringKind, StringValue};

/// Identity of a module linked into this editor. Its exports remain in a
/// private environment and are initialized before the app's global code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleId(pub(crate) u32);

struct Remapping {
    strings: Vec<u32>,
    function_base: u32,
    bigint_base: u32,
    regexp_base: u32,
    shape_base: u32,
    switch_base: u32,
    values: BTreeMap<u32, u32>,
}

impl Editor<'_> {
    /// Links a compiled v98 JavaScript module and returns its stable identity.
    /// The module must evaluate to an exports object and keep declarations in
    /// its own lexical scope (normally an IIFE). CJS segments, global variable
    /// declarations and incompatible static-builtin assumptions are refused.
    /// Every function, string, bigint, regexp, literal and shape reference is
    /// remapped. Module debug information is dropped; exceptions are retained.
    /// The shared module initializer is assembled once when the file is written.
    pub fn link(&mut self, module: &HermesFile<'_>) -> Result<ModuleId> {
        self.transaction(|editor| editor.link_module(module))
    }

    #[expect(
        clippy::too_many_lines,
        reason = "linking visits each versioned file segment in order"
    )]
    fn link_module(&mut self, module: &HermesFile<'_>) -> Result<ModuleId> {
        if module.header[18] != 0 || module.header[17] != 0 {
            return Err(HermesError::Unsupported(
                "extensions must be standalone execution bytecode, not CommonJS segments".into(),
            ));
        }
        if module.source[112] & 1 != 0 && self.file.source[112] & 1 == 0 {
            return Err(HermesError::Unsupported(
                "extension assumes static builtins but app does not".into(),
            ));
        }
        if module
            .function(module.global_function())?
            .instructions()?
            .iter()
            .any(|i| i.definition().name == "DeclareGlobalVar")
        {
            return Err(HermesError::Unsupported(
                "extension declares globals; compile an IIFE returning exports".into(),
            ));
        }
        let mut strings = Vec::with_capacity(module.string_count() as usize);
        let mut id = 0;
        for entry in module.section(Section::Kinds).as_chunks::<4>().0 {
            let run = u32::from_le_bytes(*entry);
            let kind = if run >> 31 == 0 {
                StringKind::String
            } else {
                StringKind::Identifier
            };
            for _ in 0..run & 0x7fff_ffff {
                let value = module.string(StringId(id))?;
                let (data, utf16, length) = match value {
                    StringValue::Latin1(data) => (data, false, data.len()),
                    StringValue::Utf16(data) => (data, true, data.len() / 2),
                };
                strings.push(
                    self.append_string(data.to_vec(), utf16, length as u32, kind)?
                        .0,
                );
                id += 1;
            }
        }
        let value_base = self.section_size(Section::Values) as u32;
        let key_base = self.section_size(Section::Keys) as u32;
        let (values, value_offsets) =
            remap_literals(module.section(Section::Values), &strings, value_base)?;
        let (keys, key_offsets) =
            remap_literals(module.section(Section::Keys), &strings, key_base)?;
        let remapping = Remapping {
            strings,
            function_base: self.next_function().0,
            bigint_base: (self.section_size(Section::BigInts) / 8) as u32,
            regexp_base: (self.section_size(Section::Regexps) / 8) as u32,
            shape_base: (self.section_size(Section::Shapes) / 8) as u32,
            switch_base: self.edits.string_switches,
            values: value_offsets,
        };
        self.edits.additions[Section::Values as usize].extend(values);
        self.edits.additions[Section::Keys as usize].extend(keys);
        for entry in module.section(Section::Shapes).as_chunks::<8>().0 {
            let old_offset = read_u32(entry, 0)?;
            let offset = key_offsets
                .get(&old_offset)
                .ok_or_else(|| invalid(old_offset as usize, "shape starts outside key buffer"))?;
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
                for (operand, value) in inst.definition().operands.iter().zip(&mut inst.values) {
                    *value = u64::from(match operand.id {
                        IdKind::None => continue,
                        IdKind::String => {
                            *remapping.strings.get(*value as usize).ok_or_else(|| {
                                invalid(inst.offset as usize, "string ID out of range")
                            })?
                        }
                        IdKind::Function => {
                            checked_id(*value, module.function_count(), remapping.function_base)?
                        }
                        IdKind::BigInt => {
                            checked_id(*value, module.header[9], remapping.bigint_base)?
                        }
                        IdKind::RegExp => {
                            checked_id(*value, module.header[11], remapping.regexp_base)?
                        }
                        IdKind::Shape => {
                            checked_id(*value, module.header[15], remapping.shape_base)?
                        }
                        IdKind::Switch => {
                            checked_id(*value, module.header[16], remapping.switch_base)?
                        }
                        IdKind::ValueBuffer => {
                            *remapping.values.get(&(*value as u32)).ok_or_else(|| {
                                invalid(inst.offset as usize, "literal offset out of range")
                            })?
                        }
                    });
                }
            }
            let mut edited = assemble(&function, instructions, Some(&remapping.strings))?;
            edited.header.name = remapping.strings[function.header.name as usize];
            self.append_function(edited);
        }
        self.edits.string_switches = self
            .edits
            .string_switches
            .checked_add(module.header[16])
            .ok_or_else(|| invalid(0, "too many string switches"))?;
        let module_id = ModuleId(self.edits.modules.len() as u32);
        self.edits.modules.push(FunctionId(
            remapping.function_base + module.global_function().0,
        ));
        let exports = crate::exports::exports(module)?;
        self.edits.module_exports.push(
            exports
                .into_iter()
                .map(|id| StringId(remapping.strings[id.0 as usize]))
                .collect(),
        );
        self.refresh_bootstrap()?;
        Ok(module_id)
    }

    fn section_size(&self, section: Section) -> usize {
        self.file.section(section).len() + self.edits.additions[section as usize].len()
    }

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
            instruction(
                "CreateTopLevelEnvironment",
                &[0, self.edits.modules.len() as u64],
            ),
            instruction("LoadConstUndefined", &[1]),
            instruction("LoadParam", &[2, 0]),
        ];
        for (slot, function) in self.edits.modules.iter().enumerate() {
            code.extend([
                instruction("CreateClosureLongIndex", &[3, 1, u64::from(function.0)]),
                instruction("Call1", &[3, 3, 1]),
                instruction("StoreToEnvironmentL", &[0, slot as u64, 3]),
            ]);
        }
        code.extend([
            instruction(
                "CreateClosureLongIndex",
                &[3, 0, u64::from(self.file.global_function().0)],
            ),
            instruction("Call1", &[3, 3, 2]),
            instruction("Ret", &[3]),
        ]);
        let name = StringId(
            self.edits.appended[(self.edits.global.0 - self.file.function_count()) as usize]
                .header
                .name,
        );
        Some(generated_function(name, 1, 16, encode(&code)))
    }
}

pub(crate) fn generated_function(
    name: StringId,
    parameters: u32,
    frame_size: u32,
    body: Vec<u8>,
) -> EditedFunction {
    EditedFunction {
        header: FunctionHeader {
            offset: 0,
            parameters,
            loop_depth: 0,
            size: body.len() as u32,
            name: name.0,
            number_regs: 0,
            non_pointer_regs: 0,
            frame_size,
            read_cache: 0,
            write_cache: 0,
            object_cache: 0,
            private_cache: 0,
            flags: 5,
            info_offset: 0,
        },
        body: FunctionBody::Owned(body),
        exceptions: Vec::new(),
    }
}

fn checked_id(value: u64, count: u32, base: u32) -> Result<u32> {
    if value >= u64::from(count) {
        return Err(invalid(0, "linked ID out of range"));
    }
    base.checked_add(value as u32)
        .ok_or_else(|| invalid(0, "linked ID overflow"))
}

fn remap_literals(
    source: &[u8],
    strings: &[u32],
    base: u32,
) -> Result<(Vec<u8>, BTreeMap<u32, u32>)> {
    let mut output = Vec::new();
    let mut offsets = BTreeMap::new();
    let mut cursor = 0;
    while cursor < source.len() {
        offsets.insert(cursor as u32, base + output.len() as u32);
        let tag = source[cursor];
        cursor += 1;
        let count = if tag & 0x80 == 0 {
            usize::from(tag & 15)
        } else {
            let low = slice(source, cursor, 1)?[0];
            cursor += 1;
            usize::from(tag & 15) << 8 | usize::from(low)
        };
        if count == 0 {
            return Err(invalid(cursor, "empty literal run"));
        }
        let kind = tag & 0x70;
        let stride = match kind {
            0x30 => 8,
            0x40 | 0x70 => 4,
            0x50 => 2,
            _ => 0,
        };
        let new_tag = if kind == 0x50 {
            (tag & !0x70) | 0x40
        } else {
            tag
        };
        output.push(new_tag);
        if tag & 0x80 != 0 {
            output.push(count as u8);
        }
        let data = slice(source, cursor, count * stride)?;
        if matches!(kind, 0x40 | 0x50) {
            for entry in data.chunks_exact(stride) {
                let id = if stride == 2 {
                    u32::from(u16::from_le_bytes([entry[0], entry[1]]))
                } else {
                    read_u32(entry, 0)?
                };
                let id = strings
                    .get(id as usize)
                    .ok_or_else(|| invalid(cursor, "literal string ID out of range"))?;
                output.extend_from_slice(&id.to_le_bytes());
            }
        } else {
            output.extend_from_slice(data);
        }
        cursor += data.len();
    }
    Ok((output, offsets))
}
