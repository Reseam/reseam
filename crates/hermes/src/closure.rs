// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::error::{Result, Unsupported, unsupported};
use crate::opcode::Op;
use crate::{FunctionId, FunctionIndex, HermesFile};

mod sites;

use sites::{ClosureSite, Value, analyse};

/// `target` and every function whose closures lead to it.
fn ancestors(index: &FunctionIndex, target: FunctionId) -> BTreeSet<FunctionId> {
    let mut found = BTreeSet::from([target]);
    let mut pending = VecDeque::from([target]);
    while let Some(id) = pending.pop_front() {
        for &creator in index.creators(id) {
            if found.insert(creator) {
                pending.push_back(creator);
            }
        }
    }
    found
}

/// Closure environment analyses and hook plans, kept across hooks.
#[derive(Default)]
pub(crate) struct Closures {
    analyses: BTreeMap<FunctionId, ClosureAnalysis>,
    plans: BTreeMap<FunctionId, HookPlan>,
}

impl Closures {
    /// Where the private environment of linked modules sits for `target`, and the app closures
    /// that must carry it there.
    pub(crate) fn plan(
        &mut self,
        file: &HermesFile<'_>,
        index: &FunctionIndex,
        target: FunctionId,
    ) -> Result<HookPlan> {
        if let Some(plan) = self.plans.get(&target) {
            return Ok(plan.clone());
        }
        let needed = ancestors(index, target);
        if !needed.contains(&file.global_function()) {
            return Err(unsupported(target, Unsupported::Unreachable));
        }
        let mut depths = BTreeMap::from([(file.global_function(), 0_u16)]);
        let mut work = VecDeque::from([file.global_function()]);
        let mut roots = BTreeMap::new();
        while let Some(id) = work.pop_front() {
            let depth = depths[&id];
            let analysis = self.analysis(file, id, depth)?;
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
                        return Err(unsupported(
                            id,
                            Unsupported::UnprovenEnvironment(site.offset),
                        ));
                    }
                };
                if let Some(previous) = depths.insert(site.target, child_depth) {
                    if previous != child_depth {
                        return Err(unsupported(site.target, Unsupported::EnvironmentDepths));
                    }
                } else {
                    work.push_back(site.target);
                }
            }
            if !attach.is_empty() || analysis.top_level {
                environment_depth(id, depth)?;
                roots.insert(
                    id,
                    RootAttachment {
                        depth,
                        sites: attach,
                    },
                );
            }
        }
        let depth = *depths
            .get(&target)
            .ok_or_else(|| unsupported(target, Unsupported::UnresolvedEnvironment))?;
        let plan = HookPlan { depth, roots };
        self.plans.insert(target, plan.clone());
        Ok(plan)
    }

    fn analysis(
        &mut self,
        file: &HermesFile<'_>,
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
            return Err(unsupported(id, Unsupported::EnvironmentDepths));
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

/// `depth` as the operand of an environment instruction in `function`.
pub(crate) fn environment_depth(function: FunctionId, depth: u16) -> Result<u8> {
    u8::try_from(depth).map_err(|_| unsupported(function, Unsupported::DeepEnvironment))
}
