// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::instruction::Instruction;
use crate::model::code::CodeItem;
use crate::model::{
    FieldIdx, MethodHandleIdx, MethodIdx, PerPool, Pool, ProtoIdx, StringIdx, TypeIdx,
};
mod metadata;

use crate::references::{self, RefSink, ReferenceValidator};

/// Where each source pool index lands in the written pools; `u32::MAX` marks
/// an entry the output leaves out.
pub(crate) struct Remap(PerPool<Vec<u32>>);

impl Remap {
    pub(crate) fn new(tables: PerPool<Vec<u32>>) -> Self {
        Self(tables)
    }

    pub(crate) fn into_tables(self) -> PerPool<Vec<u32>> {
        self.0
    }

    /// The written index of a source index the output must hold.
    pub(crate) fn index(&self, pool: Pool, idx: u32) -> crate::Result<u32> {
        self.0[pool]
            .get(idx as usize)
            .copied()
            .filter(|&mapped| mapped != u32::MAX)
            .ok_or_else(|| {
                crate::error::invalid(
                    "pool remap",
                    format!("{pool:?} index {idx} is absent from the output pool"),
                )
            })
    }

    /// Fails unless every reference `walk` reports has a place in the output pools.
    pub(super) fn check(
        &self,
        walk: impl FnOnce(&mut ReferenceValidator<'_>) -> crate::Result<()>,
    ) -> crate::Result<()> {
        let covered = |pool, index| self.index(pool, index).map(drop);
        let mut validator = ReferenceValidator::new(&covered);
        walk(&mut validator)?;
        validator.finish()
    }

    pub(super) fn map(&self, pool: Pool, idx: u32) -> u32 {
        self.0[pool][idx as usize]
    }

    pub(crate) fn remap_string(&self, idx: StringIdx) -> StringIdx {
        StringIdx(self.map(Pool::String, idx.0))
    }

    pub(crate) fn remap_type(&self, idx: TypeIdx) -> TypeIdx {
        TypeIdx(self.map(Pool::Type, idx.0))
    }

    pub(crate) fn remap_proto(&self, idx: ProtoIdx) -> ProtoIdx {
        ProtoIdx(self.map(Pool::Proto, idx.0))
    }

    pub(crate) fn remap_field(&self, idx: FieldIdx) -> FieldIdx {
        FieldIdx(self.map(Pool::Field, idx.0))
    }

    pub(crate) fn remap_method(&self, idx: MethodIdx) -> MethodIdx {
        MethodIdx(self.map(Pool::Method, idx.0))
    }

    pub(crate) fn remap_method_handle_idx(&self, idx: MethodHandleIdx) -> MethodHandleIdx {
        MethodHandleIdx(self.map(Pool::MethodHandle, idx.0))
    }

    pub(crate) fn remap_code(&self, code: &mut CodeItem) -> crate::error::Result<()> {
        self.check(|refs| {
            for instruction in &code.instructions {
                references::instruction(refs, instruction);
            }
            for handler in &code.catch_handlers {
                for catch in &handler.typed_catches {
                    refs.add(Pool::Type, catch.exception_type.0);
                }
            }
            if let Some(debug) = &code.debug_info {
                references::debug_info(refs, debug.read()?.as_ref());
            }
            Ok(())
        })?;

        for insn in &mut code.instructions {
            self.remap_instruction(insn);
        }
        for handler in &mut code.catch_handlers {
            for tc in &mut handler.typed_catches {
                tc.exception_type = self.remap_type(tc.exception_type);
            }
        }
        if let Some(ref mut debug) = code.debug_info {
            self.remap_debug(debug.resolve_mut()?)?;
        }
        Ok(())
    }

    fn remap_instruction(&self, insn: &mut Instruction) {
        insn.map_indices(|pool, index| self.map(pool, index));
    }
}

pub(crate) fn fixup_code(code: &mut CodeItem) -> crate::error::Result<()> {
    let mut i = 0;
    while i < code.instructions.len() {
        if let Instruction::ConstString { dest, string } = &code.instructions[i]
            && string.0 > 0xFFFF
        {
            let promoted = Instruction::ConstStringJumbo {
                dest: *dest,
                string: *string,
            };
            code.replace_instructions(i, &[promoted])?;
        }
        i += 1;
    }
    Ok(())
}
