// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Test builds that log each place a patch changed app code when the app runs it.

use std::cmp::Reverse;
use std::sync::Arc;

use reseam_dex::{
    BlockOperand, BlockRegister, BlockStep, CodeItem, DexFile, IncomingBranches, Instruction,
    InstructionMap, MethodIdx, RegList, RegisterKind, StringIdx, TypeIdx,
};

use super::{MethodLocation, PatchContext, method_mut};
use crate::error::Result;
use crate::patch::PatchRef;

const TAG: &str = "reseam-trace";

/// Where a patch changed a method: the first instruction of the code it wrote or rewrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TracePoint {
    pub patch: PatchRef,
    pub index: usize,
}

/// How an edit moved a method's instructions, which decides where a point follows them.
#[derive(Clone, Copy)]
pub(crate) enum Moved {
    /// Code went in before instructions: a point stays on its instruction, past that code, as
    /// branches that skip inserted code do.
    Insertion,
    /// Instructions were replaced, removed or expanded: a point goes to the start of what now
    /// stands in their place.
    Rewrite,
}

impl TracePoint {
    pub(crate) fn relocate(&mut self, map: &InstructionMap, moved: Moved) {
        self.index = match moved {
            Moved::Insertion => map.instructions()[self.index],
            Moved::Rewrite => map.starts()[self.index],
        };
    }
}

impl PatchContext<'_> {
    /// Adds a call before each of `points` that logs, under the tag `reseam-trace`, the patch,
    /// the method and the point's index as the method stands without these calls. Every path to
    /// a point runs its call.
    pub(crate) fn trace(
        &mut self,
        method: MethodLocation,
        mut points: Vec<TracePoint>,
    ) -> Result<()> {
        let dex = self
            .class_dex_mut(method.dex_idx, method.class_idx)?
            .expect("a traced method's class is in the app");
        let log = dex.intern_method(
            "Landroid/util/Log;",
            "i",
            "(Ljava/lang/String;Ljava/lang/String;)I",
        )?;
        let tag = dex.intern_string(TAG);
        let encoded = method_mut(dex, method)?.expect("a traced method keeps its slot");
        let (id, flags) = (encoded.method, encoded.access_flags);
        let mut code = encoded
            .code
            .take()
            .map(Arc::unwrap_or_clone)
            .expect("a traced method has code");
        let descriptor = dex.method_descriptor(id);
        let incoming = dex.incoming_types(id, flags);
        points.sort_by_key(|point| Reverse(point.index));
        let messages: Vec<StringIdx> = points
            .iter()
            .map(|point| {
                dex.intern_string(&format!("{}  {descriptor}  [{}]", point.patch, point.index))
            })
            .collect();
        let inserted = insert_calls(&mut code, points, &messages, log, tag, &incoming, dex);
        method_mut(dex, method)?
            .expect("a traced method keeps its slot")
            .code = Some(Arc::new(code));
        Ok(inserted?)
    }
}

/// Inserts each point's call, the last point first, carrying the points not yet traced through
/// what each insertion moved.
fn insert_calls(
    code: &mut CodeItem,
    mut points: Vec<TracePoint>,
    messages: &[StringIdx],
    log: MethodIdx,
    tag: StringIdx,
    incoming: &[TypeIdx],
    dex: &DexFile,
) -> reseam_dex::Result<()> {
    for (position, &message) in messages.iter().enumerate() {
        let inserted = reseam_dex::insert_block(
            code,
            points[position].index,
            &log_call(log, tag, message),
            IncomingBranches::Enter,
            incoming,
            &[],
            dex,
        )?;
        for later in &mut points[position + 1..] {
            if let Some(growth) = &inserted.growth {
                later.relocate(&growth.map, Moved::Rewrite);
            }
            later.relocate(&inserted.insertion, Moved::Insertion);
        }
    }
    Ok(())
}

/// `Log.i(tag, message)`, its two strings in registers the insertion chooses.
fn log_call(log: MethodIdx, tag: StringIdx, message: StringIdx) -> [BlockStep; 3] {
    let string = |register, string| BlockStep::Instruction {
        instruction: Instruction::ConstString { dest: 0, string },
        operands: vec![object(register)],
        target: None,
    };
    [
        string(0, tag),
        string(1, message),
        BlockStep::Instruction {
            instruction: Instruction::InvokeStatic {
                method: log,
                args: RegList::try_from(&[0, 1][..]).expect("two arguments fit a compact call"),
            },
            operands: vec![object(0), object(1)],
            target: None,
        },
    ]
}

fn object(temporary: u32) -> BlockOperand {
    BlockOperand {
        register: BlockRegister::Temporary(temporary),
        kind: RegisterKind::Object,
    }
}
