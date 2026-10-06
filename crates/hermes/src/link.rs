// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;

use crate::assemble::{assemble, encode};
use crate::edit::{EditedFunction, Editor, FunctionBody};
use crate::error::{HermesError, Result, invalid};
use crate::model::{
    Field, FunctionFlags, FunctionHeader, FunctionKind, Prohibit, STATIC_BUILTINS, Section,
    StringId, StringKind,
};
use crate::opcode::{IdKind, Instruction, Op};
use crate::parse::{read_u32, slice};
use crate::{FunctionId, HermesFile};

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
        let mut strings = Vec::with_capacity(module.string_count() as usize);
        let mut id = 0;
        for run in module.kind_runs() {
            for _ in 0..run.count {
                let value = module.string(StringId(id))?;
                let length = value.bytes.len() / value.encoding.unit_size();
                strings.push(
                    self.append_string(
                        value.bytes.to_vec(),
                        value.encoding,
                        length as u32,
                        run.kind,
                    )?
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
            bigint_base: self.section_count(Section::BigInts),
            regexp_base: self.section_count(Section::Regexps),
            shape_base: self.section_count(Section::Shapes),
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
                let at = at(inst);
                for (operand, value) in inst.definition().operands.iter().zip(&mut inst.values) {
                    *value = u64::from(match operand.id {
                        IdKind::None => continue,
                        IdKind::String => *remapping
                            .strings
                            .get(*value as usize)
                            .ok_or_else(|| invalid(at, "string ID out of range"))?,
                        IdKind::Function => {
                            checked_id(*value, module.function_count(), remapping.function_base)?
                        }
                        IdKind::BigInt => checked_id(
                            *value,
                            module.field(Field::BigIntCount),
                            remapping.bigint_base,
                        )?,
                        IdKind::RegExp => checked_id(
                            *value,
                            module.field(Field::RegExpCount),
                            remapping.regexp_base,
                        )?,
                        IdKind::Shape => checked_id(
                            *value,
                            module.field(Field::ObjectShapeCount),
                            remapping.shape_base,
                        )?,
                        IdKind::Switch => checked_id(
                            *value,
                            module.field(Field::StringSwitchCount),
                            remapping.switch_base,
                        )?,
                        IdKind::ValueBuffer => *remapping
                            .values
                            .get(&(*value as u32))
                            .ok_or_else(|| invalid(at, "literal offset out of range"))?,
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
            .checked_add(module.field(Field::StringSwitchCount))
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
        for (slot, function) in self.edits.modules.iter().enumerate() {
            code.extend([
                Instruction::new(Op::CreateClosureLongIndex, &[3, 1, u64::from(function.0)]),
                Instruction::new(Op::Call1, &[3, 3, 1]),
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

/// A strict function that refuses construction, as compiled for ordinary strict-mode code.
pub(crate) fn generated_function(
    name: StringId,
    parameters: u32,
    frame_size: u32,
    body: Vec<u8>,
) -> EditedFunction {
    EditedFunction::new(
        FunctionHeader {
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
            flags: FunctionFlags {
                prohibit: Prohibit::Construct,
                strict: true,
                exception_handler: false,
                debug_info: false,
                kind: FunctionKind::Normal,
            },
            large: None,
        },
        FunctionBody::Owned(body),
        Vec::new(),
    )
}

fn at(inst: &Instruction) -> usize {
    inst.offset.expect("decoded instructions have offsets") as usize
}

fn checked_id(value: u64, count: u32, base: u32) -> Result<u32> {
    if value >= u64::from(count) {
        return Err(invalid(0, "linked ID out of range"));
    }
    base.checked_add(value as u32)
        .ok_or_else(|| invalid(0, "linked ID overflow"))
}

/// The value type of a run in a literal buffer (`SerializedLiteralGenerator` tags).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Literal {
    Null,
    True,
    False,
    Number,
    LongString,
    ShortString,
    Undefined,
    Integer,
}

impl Literal {
    const MASK: u8 = 0x70;
    /// Set when the run length continues into a second byte.
    const LONG_RUN: u8 = 0x80;
    const LENGTH: u8 = 0x0f;

    fn decode(tag: u8) -> Self {
        match (tag & Self::MASK) >> 4 {
            0 => Self::Null,
            1 => Self::True,
            2 => Self::False,
            3 => Self::Number,
            4 => Self::LongString,
            5 => Self::ShortString,
            6 => Self::Undefined,
            _ => Self::Integer,
        }
    }

    const fn tag(self) -> u8 {
        (self as u8) << 4
    }

    const fn size(self) -> usize {
        match self {
            Self::Null | Self::True | Self::False | Self::Undefined => 0,
            Self::ShortString => 2,
            Self::LongString | Self::Integer => 4,
            Self::Number => 8,
        }
    }
}

/// Copies a literal buffer with its string IDs remapped. Short strings become
/// long strings, since remapped IDs need not fit 16 bits. Returns the copy and
/// each run's new offset by its old one.
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
        let long_run = tag & Literal::LONG_RUN != 0;
        let mut count = usize::from(tag & Literal::LENGTH);
        if long_run {
            count = count << 8 | usize::from(slice(source, cursor, 1)?[0]);
            cursor += 1;
        }
        if count == 0 {
            return Err(invalid(cursor, "empty literal run"));
        }
        let kind = Literal::decode(tag);
        let remapped = if kind == Literal::ShortString {
            Literal::LongString
        } else {
            kind
        };
        output.push(tag & !Literal::MASK | remapped.tag());
        if long_run {
            output.push(count as u8);
        }
        let data = slice(source, cursor, count * kind.size())?;
        if matches!(kind, Literal::LongString | Literal::ShortString) {
            for entry in data.chunks_exact(kind.size()) {
                let id = if kind == Literal::ShortString {
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
