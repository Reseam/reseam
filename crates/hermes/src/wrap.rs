// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::assemble::{assemble, encode};
use crate::edit::{EditedFunction, Editor, FunctionBody, Hook};
use crate::error::{HermesError, Result, invalid};
use crate::link::generated_function;
use crate::model::{FunctionKind, Prohibit, StringId, StringKind};
use crate::opcode::{BUILTIN_APPLY, BUILTIN_APPLYARGUMENTS, IdKind, Instruction, Op, OperandKind};
use crate::parse::switch_table;
use crate::{Function, FunctionId, ModuleId};

/// A value a wrap passes to its export ahead of `original`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Argument {
    Bool(bool),
    Int(i32),
    Text(String),
    /// A callable that a linked module exports.
    Export {
        module: ModuleId,
        name: String,
    },
}

#[derive(Clone)]
pub(crate) enum Bound {
    Bool(bool),
    Int(i32),
    Text(StringId),
    Export { module: ModuleId, name: StringId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Value {
    Undefined,
    Environment(u16),
}

type State = BTreeMap<u32, Value>;

#[derive(Clone)]
struct ClosureSite {
    target: FunctionId,
    instruction: usize,
    environment: Option<Value>,
    offset: u32,
}

impl Editor<'_> {
    /// Replaces the body of `function` with a call to `export(...bound, original, ...arguments)`;
    /// callers are untouched. `bound` values are fixed when the wrap is made, so one export can
    /// serve several targets. The export receives the same receiver. `original` is receiver-bound
    /// and invokes the previous wrap, or the unchanged body for the first wrap,
    /// with the original captured environment. Later wraps run outermost,
    /// including exports from different modules, in application order.
    /// Generators, async functions, constructors, globals,
    /// unknown exports and unprovable environment chains fail explicitly.
    /// Ordinary wrapped functions cannot subsequently be used as constructors.
    /// Closure ancestry and environment analysis are retained across calls.
    /// Each call adds one layer; shared ancestor bodies are assembled at write time.
    pub fn wrap(
        &mut self,
        function: FunctionId,
        module: ModuleId,
        export: &str,
        bound: &[Argument],
    ) -> Result<()> {
        self.transaction(|editor| editor.wrap_function(function, module, export, bound))
    }

    fn wrap_function(
        &mut self,
        function: FunctionId,
        module: ModuleId,
        export: &str,
        bound: &[Argument],
    ) -> Result<()> {
        if bound.len() >= usize::from(u8::MAX) {
            return Err(HermesError::Unsupported(format!(
                "{} bound arguments; at most 254 are supported",
                bound.len()
            )));
        }
        let export = self.declared_export(module, export)?;
        let bound = bound
            .iter()
            .map(|argument| {
                Ok(match argument {
                    Argument::Bool(value) => Bound::Bool(*value),
                    Argument::Int(value) => Bound::Int(*value),
                    Argument::Text(text) => Bound::Text(self.intern(text, StringKind::String)?),
                    Argument::Export { module, name } => Bound::Export {
                        module: *module,
                        name: self.declared_export(*module, name)?,
                    },
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let target = self.editable(function)?;
        let plan = self.hook_plan(function)?;
        if let Some(id) = plan
            .roots
            .keys()
            .find(|id| self.edits.discarded.contains(id) && !self.edits.relocated.contains_key(id))
        {
            return Err(HermesError::Unsupported(format!(
                "function {} returns a constant, so the closures this wrap needs no longer exist",
                id.0
            )));
        }
        let rewrap = self.edits.functions.contains_key(&function);
        let previous = if rewrap {
            self.edits.functions[&function].clone()
        } else {
            original_function(&target)?
        };
        let original = self.next_function();
        let bridge = FunctionId(original.0 + 1);
        let hook = Hook {
            module,
            export,
            bound,
        };
        let body = wrapper(&target, original, bridge, plan.depth, &hook)?;
        self.append_function(previous);
        self.append_function(bound_original(target.name()));
        self.edits.functions.insert(function, body);
        if !rewrap {
            self.edits.relocated.insert(function, original);
        }
        for (id, root) in plan.roots {
            self.edits
                .roots
                .entry(id)
                .and_modify(|existing| existing.sites.extend(&root.sites))
                .or_insert(root);
        }
        Ok(())
    }

    fn declared_export(&mut self, module: ModuleId, export: &str) -> Result<StringId> {
        if module.0 as usize >= self.edits.modules.len() {
            return Err(invalid(0, "module belongs to another editor"));
        }
        let declared = self.edits.module_exports[module.0 as usize]
            .iter()
            .any(|id| {
                self.edits.strings[(id.0 - self.file.string_count()) as usize]
                    .value()
                    .equals(export)
            });
        if !declared {
            return Err(HermesError::Unsupported(format!(
                "module does not statically export callable {export:?}"
            )));
        }
        self.intern(export, StringKind::Identifier)
    }

    fn hook_plan(&mut self, target: FunctionId) -> Result<HookPlan> {
        if self.edits.hook_graph.is_none() {
            self.edits.hook_graph = Some(HookGraph::build(&self.file)?);
        }
        let graph = self
            .edits
            .hook_graph
            .as_mut()
            .expect("hook graph initialized");
        if let Some(plan) = graph.plans.get(&target) {
            return Ok(plan.clone());
        }
        let mut needed = BTreeSet::from([target]);
        let mut pending = VecDeque::from([target]);
        while let Some(id) = pending.pop_front() {
            if let Some(creators) = graph.parents.get(&id) {
                for &creator in creators {
                    if needed.insert(creator) {
                        pending.push_back(creator);
                    }
                }
            }
        }
        if !needed.contains(&self.file.global_function()) {
            return Err(HermesError::Unsupported(
                "wrapped functions are not reachable from app global closures".into(),
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
                u8::try_from(depth).map_err(|_| {
                    HermesError::Unsupported("private environment deeper than 255 scopes".into())
                })?;
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

    pub(crate) fn rooted_functions(&self) -> Result<BTreeMap<FunctionId, EditedFunction>> {
        self.edits
            .roots
            .iter()
            .map(|(&id, root)| {
                let function = self.file.function(id)?;
                let body =
                    attach_root(&function, function.instructions()?, root.depth, &root.sites)?;
                // A wrapped function's original body lives on in the function its wrapper calls.
                Ok((self.edits.relocated.get(&id).copied().unwrap_or(id), body))
            })
            .collect()
    }
}

impl<'a> Editor<'a> {
    /// The app function whose body an edit may replace. Generators, async functions and
    /// constructors keep state or a receiver that a replaced body cannot reproduce.
    pub(crate) fn editable(&self, function: FunctionId) -> Result<Function<'a>> {
        if function == self.file.global_function() {
            return Err(HermesError::Unsupported("global function".into()));
        }
        let target = self.file.function(function)?;
        if target.header.flags.kind != FunctionKind::Normal
            || target.header.flags.prohibit == Prohibit::Call
            || target
                .instructions()?
                .iter()
                .any(|i| matches!(i.op, Op::GetNewTarget | Op::DirectEval))
        {
            return Err(HermesError::Unsupported(format!(
                "function {} is a generator, async function, constructor or uses new.target/eval",
                function.0
            )));
        }
        Ok(target)
    }
}

pub(crate) struct HookGraph {
    parents: BTreeMap<FunctionId, BTreeSet<FunctionId>>,
    analyses: BTreeMap<FunctionId, ClosureAnalysis>,
    plans: BTreeMap<FunctionId, HookPlan>,
}

impl HookGraph {
    fn build(file: &crate::HermesFile<'_>) -> Result<Self> {
        let mut parents = BTreeMap::<FunctionId, BTreeSet<FunctionId>>::new();
        for id in (0..file.function_count()).map(FunctionId) {
            let function = file.function(id)?;
            for inst in function.instructions()? {
                for (operand, value) in inst.operands() {
                    if operand.id == IdKind::Function {
                        parents
                            .entry(FunctionId(value as u32))
                            .or_default()
                            .insert(id);
                    }
                }
            }
        }
        Ok(Self {
            parents,
            analyses: BTreeMap::new(),
            plans: BTreeMap::new(),
        })
    }

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
    depth: u16,
    sites: BTreeSet<usize>,
}

#[derive(Clone)]
struct HookPlan {
    depth: u16,
    roots: BTreeMap<FunctionId, RootAttachment>,
}

/// Merges `state` into the entry state of `next`, queueing it when that changes.
fn flow(
    states: &mut BTreeMap<usize, State>,
    work: &mut VecDeque<usize>,
    next: usize,
    state: &State,
) {
    if let Some(existing) = states.get_mut(&next) {
        let before = existing.len();
        existing.retain(|reg, value| state.get(reg) == Some(value));
        if existing.len() != before {
            work.push_back(next);
        }
    } else {
        states.insert(next, state.clone());
        work.push_back(next);
    }
}

fn successors(
    function: &Function<'_>,
    code: &[Instruction],
    index: usize,
    indices: &BTreeMap<u32, usize>,
) -> Result<Vec<usize>> {
    let inst = &code[index];
    let origin = i64::from(inst.offset.expect("analysed instructions are decoded"));
    let mut result = Vec::new();
    let mut add = |relative: i64| -> Result<()> {
        let target = u32::try_from(origin + relative)
            .map_err(|_| invalid(origin as usize, "invalid environment-analysis branch"))?;
        result.push(
            *indices
                .get(&target)
                .ok_or_else(|| invalid(target as usize, "branch outside instructions"))?,
        );
        Ok(())
    };
    for (operand, value) in inst.operands() {
        match operand.kind {
            OperandKind::Addr8 => add(i64::from(value as i8))?,
            OperandKind::Addr32 => add(i64::from(value as i32))?,
            _ => {}
        }
    }
    if let Some(table) = switch_table(function, inst)? {
        for (_, target) in table.entries() {
            add(i64::from(target))?;
        }
    }
    if !matches!(
        inst.op,
        Op::Ret
            | Op::Throw
            | Op::Unreachable
            | Op::Jmp
            | Op::JmpLong
            | Op::UIntSwitchImm
            | Op::StringSwitchImm
    ) && index + 1 < code.len()
    {
        result.push(index + 1);
    }
    Ok(result)
}

/// Tracks which registers hold `undefined` or a known environment depth, merging
/// control-flow paths conservatively, and reports what each closure captures.
fn analyse(
    function: &Function<'_>,
    code: &[Instruction],
    enclosing: u16,
) -> Result<Vec<ClosureSite>> {
    if code.is_empty() {
        return Ok(Vec::new());
    }
    let indices: BTreeMap<_, _> = code
        .iter()
        .enumerate()
        .map(|(i, inst)| (inst.offset.expect("analysed instructions are decoded"), i))
        .collect();
    let handlers = function.exception_handlers()?;
    let mut states = BTreeMap::from([(0_usize, State::new())]);
    let mut work = VecDeque::from([0_usize]);
    while let Some(index) = work.pop_front() {
        let before = states[&index].clone();
        let inst = &code[index];
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
        let value = match inst.op {
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
        };
        // A throwing instruction may have written any of its registers.
        let mut thrown = before;
        for register in inst.written_registers() {
            thrown.remove(&register);
        }
        let mut after = thrown.clone();
        if let Some(value) = value {
            after.insert(v[0] as u32, value);
        }
        for next in successors(function, code, index, &indices)? {
            flow(&mut states, &mut work, next, &after);
        }
        for handler in handlers.iter().filter(|h| h.covers(offset)) {
            let next = *indices
                .get(&handler.target)
                .ok_or_else(|| invalid(offset as usize, "invalid exception target"))?;
            flow(&mut states, &mut work, next, &thrown);
        }
    }
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

fn attach_root(
    function: &Function<'_>,
    code: Vec<Instruction>,
    depth: u16,
    attach: &BTreeSet<usize>,
) -> Result<EditedFunction> {
    let depth = u8::try_from(depth).map_err(|_| {
        HermesError::Unsupported("private environment deeper than 255 scopes".into())
    })?;
    let mut rewritten = vec![Instruction::new(
        Op::GetParentEnvironment,
        &[0, u64::from(depth)],
    )];
    for (index, mut inst) in code.into_iter().enumerate() {
        for (operand, value) in inst.definition().operands.iter().zip(&mut inst.values) {
            if operand.kind.is_register() {
                *value += 1;
            }
        }
        if attach.contains(&index) {
            inst.values[1] = 0;
        }
        if inst.op == Op::CreateTopLevelEnvironment {
            let mut replacement =
                Instruction::new(Op::CreateEnvironment, &[inst.values[0], 0, inst.values[1]]);
            replacement.offset = inst.offset;
            rewritten.push(replacement);
        } else {
            rewritten.push(inst);
        }
    }
    let mut function = assemble(function, rewritten, None)?;
    function.header.frame_size += 1;
    function.header.number_regs = 0;
    function.header.non_pointer_regs = 0;
    Ok(function)
}

fn original_function(function: &Function<'_>) -> Result<EditedFunction> {
    let mut end = function.body_range().end;
    for inst in function.instructions()? {
        if let Some(table) = switch_table(function, &inst)? {
            end = end.max(table.offset + table.bytes.len());
        }
    }
    Ok(EditedFunction::new(
        function.header.clone(),
        FunctionBody::Original(function.body_range().start..end),
        function.exception_handlers()?,
    ))
}

fn bound_original(name: StringId) -> EditedFunction {
    let code = [
        Instruction::new(Op::GetParentEnvironment, &[0, 0]),
        Instruction::new(Op::LoadFromEnvironment, &[11, 0, 0]),
        Instruction::new(Op::LoadFromEnvironment, &[10, 0, 1]),
        Instruction::new(Op::LoadConstUndefined, &[9]),
        Instruction::new(Op::CallBuiltin, &[0, u64::from(BUILTIN_APPLYARGUMENTS), 4]),
        Instruction::new(Op::Ret, &[0]),
    ];
    generated_function(name, 1, 20, encode(&code))
}

/// Appends `bound` to the argument array in r7, using r12 and r3 as scratch.
/// Returns the read cache slots the wrapper uses; slot 0 belongs to the hook export.
fn load_bound(code: &mut Vec<Instruction>, bound: &[Bound], depth: u8) -> u8 {
    let mut read_cache = 1_u8;
    for (index, value) in bound.iter().enumerate() {
        match value {
            Bound::Bool(true) => code.push(Instruction::new(Op::LoadConstTrue, &[12])),
            Bound::Bool(false) => code.push(Instruction::new(Op::LoadConstFalse, &[12])),
            Bound::Int(value) => {
                code.push(Instruction::new(
                    Op::LoadConstInt,
                    &[12, u64::from(value.cast_unsigned())],
                ));
            }
            Bound::Text(text) => {
                code.push(Instruction::new(
                    Op::LoadConstStringLongIndex,
                    &[12, u64::from(text.0)],
                ));
            }
            // Each property read needs its own cache slot.
            Bound::Export { module, name } => {
                code.extend([
                    Instruction::new(Op::GetParentEnvironment, &[3, u64::from(depth)]),
                    Instruction::new(Op::LoadFromEnvironmentL, &[3, 3, u64::from(module.0)]),
                    Instruction::new(
                        Op::GetByIdLong,
                        &[12, 3, u64::from(read_cache), u64::from(name.0)],
                    ),
                ]);
                read_cache += 1;
            }
        }
        code.push(Instruction::new(
            Op::DefineOwnByIndex,
            &[7, 12, index as u64],
        ));
    }
    read_cache
}

fn wrapper(
    function: &Function<'_>,
    original: FunctionId,
    bridge: FunctionId,
    depth: u16,
    hook: &Hook,
) -> Result<EditedFunction> {
    let depth = u8::try_from(depth).map_err(|_| {
        HermesError::Unsupported("private environment deeper than 255 scopes".into())
    })?;
    let strict = function.header.flags.strict;
    let mut code = vec![
        Instruction::new(Op::GetParentEnvironment, &[0, 0]),
        Instruction::new(Op::CreateClosureLongIndex, &[1, 0, u64::from(original.0)]),
        if strict {
            Instruction::new(Op::LoadParam, &[2, 0])
        } else {
            Instruction::new(Op::LoadThisNS, &[2])
        },
        Instruction::new(Op::GetParentEnvironment, &[3, u64::from(depth)]),
        Instruction::new(Op::LoadFromEnvironmentL, &[3, 3, u64::from(hook.module.0)]),
        Instruction::new(Op::GetByIdLong, &[4, 3, 0, u64::from(hook.export.0)]),
        Instruction::new(Op::CreateFunctionEnvironment, &[5, 2]),
        Instruction::new(Op::StoreToEnvironment, &[5, 0, 1]),
        Instruction::new(Op::StoreToEnvironment, &[5, 1, 2]),
        Instruction::new(Op::CreateClosureLongIndex, &[6, 5, u64::from(bridge.0)]),
        Instruction::new(Op::NewArray, &[7, 0]),
    ];
    let read_cache = load_bound(&mut code, &hook.bound, depth);
    let original_index = hook.bound.len() as u64;
    code.extend([
        Instruction::new(Op::DefineOwnByIndex, &[7, 6, original_index]),
        Instruction::new(Op::LoadConstUndefined, &[0]),
        Instruction::new(Op::GetArgumentsLength, &[8, 0]),
        Instruction::new(Op::LoadConstZero, &[9]),
        Instruction::new(Op::LoadConstUInt8, &[11, 1]),
        Instruction::new(Op::LoadConstUInt8, &[10, original_index + 1]),
    ]);
    let test = code.len();
    code.push(Instruction::new(Op::JGreaterEqualLong, &[0, 9, 8]));
    let loop_start = code.len();
    code.extend([
        Instruction::new(Op::GetArgumentsPropByValStrict, &[12, 9, 0]),
        Instruction::new(Op::DefineOwnByVal, &[7, 12, 10, 1]),
        Instruction::new(Op::Add, &[10, 10, 11]),
        Instruction::new(Op::Add, &[9, 9, 11]),
        Instruction::new(Op::JLessLong, &[0, 9, 8]),
    ]);
    let loop_branch = code.len() - 1;
    let done = code.len();
    code.extend([
        Instruction::new(Op::Mov, &[15, 4]),
        Instruction::new(Op::Mov, &[14, 7]),
        Instruction::new(Op::Mov, &[13, 2]),
        Instruction::new(Op::CallBuiltin, &[0, u64::from(BUILTIN_APPLY), 4]),
        Instruction::new(Op::Ret, &[0]),
    ]);
    let positions: Vec<u32> = code
        .iter()
        .scan(0, |position, inst| {
            let start = *position;
            *position += inst.definition().size() as u32;
            Some(start)
        })
        .collect();
    let relative = |from: usize, to: usize| u64::from(positions[to].wrapping_sub(positions[from]));
    code[test].values[0] = relative(test, done);
    code[loop_branch].values[0] = relative(loop_branch, loop_start);
    let mut result = generated_function(
        function.name(),
        function.header.parameters,
        24,
        encode(&code),
    );
    result.header.read_cache = read_cache;
    result.header.flags.strict = strict;
    Ok(result)
}
