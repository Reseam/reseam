// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::dataflow::{Registers, register_states};
use crate::edit::Editor;
use crate::error::{HermesError, Result, invalid};
use crate::function::Function;
use crate::opcode::{Instruction, Op};
use crate::{FunctionId, FunctionIndex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Value {
    Undefined,
    Environment(u16),
}

#[derive(Clone)]
struct ClosureSite {
    target: FunctionId,
    instruction: usize,
    environment: Option<Value>,
    offset: u32,
}

impl Editor<'_> {
    /// Where the private environment of linked modules sits for `target`, and the app closures
    /// that must carry it there.
    pub(crate) fn hook_plan(
        &mut self,
        index: &FunctionIndex,
        target: FunctionId,
    ) -> Result<HookPlan> {
        let graph = self.edits.hook_graph.get_or_insert_with(HookGraph::default);
        if let Some(plan) = graph.plans.get(&target) {
            return Ok(plan.clone());
        }
        let mut needed = BTreeSet::from([target]);
        let mut pending = VecDeque::from([target]);
        while let Some(id) = pending.pop_front() {
            for &creator in index.creators(id) {
                if needed.insert(creator) {
                    pending.push_back(creator);
                }
            }
        }
        if !needed.contains(&self.file.global_function()) {
            return Err(HermesError::Unsupported(
                "hooked functions are not reachable from app global closures".into(),
            ));
        }
        let mut depths = BTreeMap::from([(self.file.global_function(), 0_u16)]);
        let mut work = VecDeque::from([self.file.global_function()]);
        let mut roots = BTreeMap::new();
        while let Some(id) = work.pop_front() {
            let depth = depths[&id];
            let analysis = graph.analysis(&self.file, id, depth)?;
            let mut attach = BTreeSet::new();
            for site in needed
                .iter()
                .filter_map(|id| analysis.sites.get(id))
                .flatten()
            {
                let child_depth = match site.environment {
                    Some(Value::Undefined) => {
                        attach.insert(site.instruction);
                        0
                    }
                    Some(Value::Environment(depth)) => depth,
                    None => {
                        return Err(HermesError::Unsupported(format!(
                            "cannot prove closure environment at function {}, byte {}",
                            id.0, site.offset
                        )));
                    }
                };
                if let Some(previous) = depths.insert(site.target, child_depth) {
                    if previous != child_depth {
                        return Err(HermesError::Unsupported(format!(
                            "function {} has closures at different environment depths",
                            site.target.0
                        )));
                    }
                } else {
                    work.push_back(site.target);
                }
            }
            if !attach.is_empty() || analysis.top_level {
                environment_depth(depth)?;
                roots.insert(
                    id,
                    RootAttachment {
                        depth,
                        sites: attach,
                    },
                );
            }
        }
        let depth = *depths.get(&target).ok_or_else(|| {
            HermesError::Unsupported(format!(
                "cannot resolve private environment for function {}",
                target.0
            ))
        })?;
        let plan = HookPlan { depth, roots };
        graph.plans.insert(target, plan.clone());
        Ok(plan)
    }
}

/// Closure environment analyses and hook plans, kept across hooks.
#[derive(Default)]
pub(crate) struct HookGraph {
    analyses: BTreeMap<FunctionId, ClosureAnalysis>,
    plans: BTreeMap<FunctionId, HookPlan>,
}

impl HookGraph {
    fn analysis(
        &mut self,
        file: &crate::HermesFile<'_>,
        id: FunctionId,
        depth: u16,
    ) -> Result<&ClosureAnalysis> {
        let analysis = match self.analyses.entry(id) {
            std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::btree_map::Entry::Vacant(entry) => {
                let function = file.function(id)?;
                let code = function.instructions()?;
                let mut sites = BTreeMap::<_, Vec<_>>::new();
                for site in analyse(&function, &code, depth)? {
                    sites.entry(site.target).or_default().push(site);
                }
                entry.insert(ClosureAnalysis {
                    depth,
                    sites,
                    top_level: code.iter().any(|i| i.op == Op::CreateTopLevelEnvironment),
                })
            }
        };
        if analysis.depth != depth {
            return Err(HermesError::Unsupported(format!(
                "function {} has closures at different environment depths",
                id.0
            )));
        }
        Ok(analysis)
    }
}

struct ClosureAnalysis {
    depth: u16,
    sites: BTreeMap<FunctionId, Vec<ClosureSite>>,
    top_level: bool,
}

#[derive(Clone)]
pub(crate) struct RootAttachment {
    pub depth: u16,
    pub sites: BTreeSet<usize>,
}

#[derive(Clone)]
pub(crate) struct HookPlan {
    pub depth: u16,
    pub roots: BTreeMap<FunctionId, RootAttachment>,
}

/// Tracks which registers hold `undefined` or a known environment depth and reports what each
/// closure captures.
fn analyse(
    function: &Function<'_>,
    code: &[Instruction],
    enclosing: u16,
) -> Result<Vec<ClosureSite>> {
    let states = register_states(function, code, |inst, before: &Registers<Value>| {
        let offset = inst.offset.expect("analysed instructions are decoded");
        let v = &inst.values;
        let shifted = |value: Value, delta: i32| -> Result<Value> {
            match value {
                Value::Environment(depth) => {
                    let depth = i32::from(depth) + delta;
                    let depth = u16::try_from(depth).map_err(|_| {
                        invalid(offset as usize, "environment escapes private root")
                    })?;
                    Ok(Value::Environment(depth))
                }
                Value::Undefined => Err(invalid(offset as usize, "undefined environment parent")),
            }
        };
        Ok(match inst.op {
            Op::LoadConstUndefined => Some(Value::Undefined),
            Op::Mov | Op::MovLong => before.get(&(v[1] as u32)).copied(),
            Op::GetParentEnvironment => {
                Some(shifted(Value::Environment(enclosing), -(v[1] as i32))?)
            }
            Op::CreateFunctionEnvironment => Some(shifted(Value::Environment(enclosing), 1)?),
            Op::CreateTopLevelEnvironment => Some(Value::Environment(1)),
            Op::CreateEnvironment => before
                .get(&(v[1] as u32))
                .copied()
                .map(|p| shifted(p, 1))
                .transpose()?,
            Op::GetEnvironment => before
                .get(&(v[1] as u32))
                .copied()
                .map(|p| shifted(p, -(v[2] as i32)))
                .transpose()?,
            _ => None,
        })
    })?;
    let mut sites = Vec::new();
    for (index, inst) in code.iter().enumerate() {
        if !matches!(inst.op, Op::CreateClosure | Op::CreateClosureLongIndex) {
            continue;
        }
        let Some(state) = states.get(&index) else {
            continue;
        };
        sites.push(ClosureSite {
            target: FunctionId(inst.values[2] as u32),
            instruction: index,
            environment: state.get(&(inst.values[1] as u32)).copied(),
            offset: inst.offset.expect("analysed instructions are decoded"),
        });
    }
    Ok(sites)
}

/// `depth` as the operand of an environment instruction.
pub(crate) fn environment_depth(depth: u16) -> Result<u8> {
    u8::try_from(depth)
        .map_err(|_| HermesError::Unsupported("private environment deeper than 255 scopes".into()))
}
