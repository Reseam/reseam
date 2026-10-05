use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::assemble::{assemble, encode, instruction};
use crate::edit::{EditedFunction, Editor, FunctionBody, Hook};
use crate::error::{HermesError, Result, invalid};
use crate::link::generated_function;
use crate::opcode::{IdKind, Instruction, OperandKind};
use crate::parse::{read_u32, slice, switch_table};
use crate::{Function, FunctionId, ModuleId, StringKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Value {
    Undefined,
    Environment(u16),
}

type State = BTreeMap<u32, Value>;

struct ClosureSite {
    target: FunctionId,
    instruction: usize,
    environment: Value,
}

impl Editor<'_> {
    /// Replaces calls to `function` with `export(original, ...arguments)`.
    /// The export receives the same receiver. `original` is receiver-bound and
    /// invokes the previous wrap, or the unchanged body for the first wrap,
    /// with the original captured environment. Later wraps run outermost,
    /// including exports from different modules, in application order.
    /// Generators, async functions, constructors, globals,
    /// unknown exports and unprovable environment chains fail explicitly.
    /// Ordinary wrapped functions cannot subsequently be used as constructors.
    pub fn wrap(&mut self, function: FunctionId, module: ModuleId, export: &str) -> Result<()> {
        self.transaction(|editor| editor.wrap_function(function, module, export))
    }

    fn wrap_function(
        &mut self,
        function: FunctionId,
        module: ModuleId,
        export: &str,
    ) -> Result<()> {
        if module.0 as usize >= self.edits.modules.len() {
            return Err(invalid(0, "module belongs to another editor"));
        }
        let declared = self.edits.module_exports[module.0 as usize]
            .iter()
            .any(|id| {
                let added = &self.edits.strings[(id.0 - self.file.string_count()) as usize];
                let value = if added.utf16 {
                    crate::StringValue::Utf16(&added.value)
                } else {
                    crate::StringValue::Latin1(&added.value)
                };
                value.equals(export)
            });
        if !declared {
            return Err(HermesError::Unsupported(format!(
                "module does not statically export callable {export:?}"
            )));
        }
        if function == self.file.global_function() {
            return Err(HermesError::Unsupported("global function".into()));
        }
        let target = self.file.function(function)?;
        if target.header.flags >> 6 != 0
            || target.header.flags.trailing_zeros() >= 2
            || target
                .instructions()?
                .iter()
                .any(|i| matches!(i.definition().name, "GetNewTarget" | "DirectEval"))
        {
            return Err(HermesError::Unsupported(format!(
                "function {} is a generator, async function, constructor or uses new.target/eval",
                function.0
            )));
        }
        let export = self.intern(export, StringKind::Identifier)?;
        let previous = if self.edits.hooks.contains_key(&function) {
            self.edits.functions[&function].clone()
        } else {
            original_function(&target)?
        };
        let original = self.append_function(previous);
        let bridge = self.append_function(bound_original(target.name()));
        self.edits.hooks.entry(function).or_default().push(Hook {
            module,
            export,
            original,
            bridge,
        });
        self.install_hooks()
    }

    fn install_hooks(&mut self) -> Result<()> {
        let mut parents = BTreeMap::<FunctionId, BTreeSet<FunctionId>>::new();
        for id in 0..self.file.function_count() {
            let function = self.file.function(FunctionId(id))?;
            for inst in function.instructions()? {
                for (operand, value) in inst.definition().operands.iter().zip(inst.values) {
                    if operand.id == IdKind::Function {
                        parents
                            .entry(FunctionId(value as u32))
                            .or_default()
                            .insert(function.id);
                    }
                }
            }
        }
        let mut needed: BTreeSet<_> = self.edits.hooks.keys().copied().collect();
        let mut pending: VecDeque<_> = needed.iter().copied().collect();
        while let Some(id) = pending.pop_front() {
            if let Some(creators) = parents.get(&id) {
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
        let mut changed = BTreeMap::new();
        while let Some(id) = work.pop_front() {
            let function = self.file.function(id)?;
            let code = function.instructions()?;
            let sites = analyse(&function, &code, depths[&id], &needed)?;
            let mut attach = BTreeSet::new();
            for site in sites.iter().filter(|site| needed.contains(&site.target)) {
                let depth = match site.environment {
                    Value::Undefined => {
                        attach.insert(site.instruction);
                        0
                    }
                    Value::Environment(depth) => depth,
                };
                if let Some(previous) = depths.insert(site.target, depth) {
                    if previous != depth {
                        return Err(HermesError::Unsupported(format!(
                            "function {} has closures at different environment depths",
                            site.target.0
                        )));
                    }
                } else {
                    work.push_back(site.target);
                }
            }
            if !attach.is_empty()
                || code
                    .iter()
                    .any(|i| i.definition().name == "CreateTopLevelEnvironment")
            {
                if self.edits.hooks.contains_key(&id) {
                    return Err(HermesError::Unsupported(
                        "a wrapped body also needs to carry a disconnected nested hook environment"
                            .into(),
                    ));
                }
                changed.insert(id, attach_root(&function, code, depths[&id], &attach)?);
            }
        }
        let mut layers = Vec::new();
        for (&id, hooks) in &self.edits.hooks {
            let depth = *depths.get(&id).ok_or_else(|| {
                HermesError::Unsupported(format!(
                    "cannot resolve private environment for function {}",
                    id.0
                ))
            })?;
            let function = self.file.function(id)?;
            for (index, hook) in hooks.iter().enumerate() {
                let body = wrapper(&function, hook.original, hook.bridge, depth, hook)?;
                if let Some(next) = hooks.get(index + 1) {
                    layers.push((next.original, body));
                } else {
                    changed.insert(id, body);
                }
            }
        }
        for (id, body) in layers {
            self.edits.appended[(id.0 - self.file.function_count()) as usize] = body;
        }
        self.edits.functions = changed;
        Ok(())
    }
}

fn merge(existing: &mut State, incoming: &State) -> bool {
    let before = existing.len();
    existing.retain(|reg, value| incoming.get(reg) == Some(value));
    existing.len() != before
}

fn successors(
    function: &Function<'_>,
    code: &[Instruction],
    index: usize,
    indices: &BTreeMap<u32, usize>,
) -> Result<Vec<usize>> {
    let inst = &code[index];
    let name = inst.definition().name;
    let mut result = Vec::new();
    let mut add = |relative: i64| -> Result<()> {
        let target = u32::try_from(i64::from(inst.offset) + relative)
            .map_err(|_| invalid(inst.offset as usize, "invalid environment-analysis branch"))?;
        result.push(
            *indices
                .get(&target)
                .ok_or_else(|| invalid(target as usize, "branch outside instructions"))?,
        );
        Ok(())
    };
    for (operand, &value) in inst.definition().operands.iter().zip(&inst.values) {
        match operand.kind {
            OperandKind::Addr8 => add(i64::from(value as i8))?,
            OperandKind::Addr32 => add(i64::from(value as i32))?,
            _ => {}
        }
    }
    if let Some(table) = switch_table(function, inst)? {
        for entry in table.bytes.chunks_exact(table.stride) {
            add(i64::from(read_u32(entry, table.stride - 4)? as i32))?;
        }
    }
    if !matches!(
        name,
        "Ret" | "Throw" | "Unreachable" | "Jmp" | "JmpLong" | "UIntSwitchImm" | "StringSwitchImm"
    ) && index + 1 < code.len()
    {
        result.push(index + 1);
    }
    Ok(result)
}

#[expect(
    clippy::too_many_lines,
    reason = "environment opcodes share a conservative dataflow transfer and merge"
)]
fn analyse(
    function: &Function<'_>,
    code: &[Instruction],
    enclosing: u16,
    needed: &BTreeSet<FunctionId>,
) -> Result<Vec<ClosureSite>> {
    if code.is_empty() {
        return Ok(Vec::new());
    }
    let indices: BTreeMap<_, _> = code
        .iter()
        .enumerate()
        .map(|(i, inst)| (inst.offset, i))
        .collect();
    let mut states = BTreeMap::from([(0_usize, State::new())]);
    let mut work = VecDeque::from([0_usize]);
    while let Some(index) = work.pop_front() {
        let mut state = states[&index].clone();
        let inst = &code[index];
        let v = &inst.values;
        let name = inst.definition().name;
        let shifted = |value: Value, delta: i32| -> Result<Value> {
            match value {
                Value::Environment(depth) => {
                    let depth = i32::from(depth) + delta;
                    let depth = u16::try_from(depth).map_err(|_| {
                        invalid(inst.offset as usize, "environment escapes private root")
                    })?;
                    Ok(Value::Environment(depth))
                }
                Value::Undefined => Err(invalid(
                    inst.offset as usize,
                    "undefined environment parent",
                )),
            }
        };
        let value = match name {
            "LoadConstUndefined" => Some(Value::Undefined),
            "Mov" | "MovLong" => state.get(&(v[1] as u32)).copied(),
            "GetParentEnvironment" => Some(shifted(Value::Environment(enclosing), -(v[1] as i32))?),
            "CreateFunctionEnvironment" => Some(shifted(Value::Environment(enclosing), 1)?),
            "CreateTopLevelEnvironment" => Some(Value::Environment(1)),
            "CreateEnvironment" => state
                .get(&(v[1] as u32))
                .copied()
                .map(|p| shifted(p, 1))
                .transpose()?,
            "GetEnvironment" => state
                .get(&(v[1] as u32))
                .copied()
                .map(|p| shifted(p, -(v[2] as i32)))
                .transpose()?,
            _ => None,
        };
        let writes_first = inst.definition().writes_first_register();
        if writes_first {
            let register = v[0] as u32;
            if let Some(value) = value {
                state.insert(register, value);
            } else {
                state.remove(&register);
            }
        }
        for next in successors(function, code, index, &indices)? {
            if let Some(existing) = states.get_mut(&next) {
                if merge(existing, &state) {
                    work.push_back(next);
                }
            } else {
                states.insert(next, state.clone());
                work.push_back(next);
            }
        }
        if function.header.flags & 8 != 0 {
            let count = read_u32(function.source, function.header.info_offset)?;
            for handler in 0..count as usize {
                let entry = slice(
                    function.source,
                    function.header.info_offset + 4 + handler * 12,
                    12,
                )?;
                if inst.offset >= read_u32(entry, 0)? && inst.offset < read_u32(entry, 4)? {
                    let next = *indices
                        .get(&read_u32(entry, 8)?)
                        .ok_or_else(|| invalid(inst.offset as usize, "invalid exception target"))?;
                    if let Some(existing) = states.get_mut(&next) {
                        if merge(existing, &state) {
                            work.push_back(next);
                        }
                    } else {
                        states.insert(next, state.clone());
                        work.push_back(next);
                    }
                }
            }
        }
    }
    let mut sites = Vec::new();
    for (index, inst) in code.iter().enumerate() {
        if !matches!(
            inst.definition().name,
            "CreateClosure" | "CreateClosureLongIndex"
        ) {
            continue;
        }
        if !needed.contains(&FunctionId(inst.values[2] as u32)) {
            continue;
        }
        let Some(state) = states.get(&index) else {
            continue;
        };
        let environment = state
            .get(&(inst.values[1] as u32))
            .copied()
            .ok_or_else(|| {
                HermesError::Unsupported(format!(
                    "cannot prove closure environment at function {}, byte {}",
                    function.id.0, inst.offset
                ))
            })?;
        sites.push(ClosureSite {
            target: FunctionId(inst.values[2] as u32),
            instruction: index,
            environment,
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
    let mut rewritten = vec![instruction("GetParentEnvironment", &[0, u64::from(depth)])];
    for (index, mut inst) in code.into_iter().enumerate() {
        for (operand, value) in inst.definition().operands.iter().zip(&mut inst.values) {
            if matches!(operand.kind, OperandKind::Reg8 | OperandKind::Reg32) {
                *value += 1;
            }
        }
        if attach.contains(&index) {
            inst.values[1] = 0;
        }
        if inst.definition().name == "CreateTopLevelEnvironment" {
            let mut replacement =
                instruction("CreateEnvironment", &[inst.values[0], 0, inst.values[1]]);
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
    let mut end = function.header.offset as usize + function.header.size as usize;
    for inst in function.instructions()? {
        if let Some(table) = switch_table(function, &inst)? {
            end = end.max(table.offset + table.bytes.len());
        }
    }
    let mut exceptions = Vec::new();
    if function.header.flags & 8 != 0 {
        let count = read_u32(function.source, function.header.info_offset)?;
        for index in 0..count as usize {
            let entry = slice(
                function.source,
                function.header.info_offset + 4 + index * 12,
                12,
            )?;
            exceptions.push([
                read_u32(entry, 0)?,
                read_u32(entry, 4)?,
                read_u32(entry, 8)?,
            ]);
        }
    }
    let mut header = function.header.clone();
    header.flags &= !0x30;
    Ok(EditedFunction {
        header,
        body: FunctionBody::Original(function.header.offset as usize..end),
        exceptions,
    })
}

fn bound_original(name: crate::StringId) -> EditedFunction {
    let code = [
        instruction("GetParentEnvironment", &[0, 0]),
        instruction("LoadFromEnvironment", &[11, 0, 0]),
        instruction("LoadFromEnvironment", &[10, 0, 1]),
        instruction("LoadConstUndefined", &[9]),
        instruction(
            "CallBuiltin",
            &[0, u64::from(crate::opcode::BUILTIN_APPLYARGUMENTS), 4],
        ),
        instruction("Ret", &[0]),
    ];
    generated_function(name, 1, 20, encode(&code))
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
    let mut code = vec![
        instruction("GetParentEnvironment", &[0, 0]),
        instruction("CreateClosureLongIndex", &[1, 0, u64::from(original.0)]),
        if function.header.flags & 4 != 0 {
            instruction("LoadParam", &[2, 0])
        } else {
            instruction("LoadThisNS", &[2])
        },
        instruction("GetParentEnvironment", &[3, u64::from(depth)]),
        instruction("LoadFromEnvironmentL", &[3, 3, u64::from(hook.module.0)]),
        instruction("GetByIdLong", &[4, 3, 0, u64::from(hook.export.0)]),
        instruction("CreateFunctionEnvironment", &[5, 2]),
        instruction("StoreToEnvironment", &[5, 0, 1]),
        instruction("StoreToEnvironment", &[5, 1, 2]),
        instruction("CreateClosureLongIndex", &[6, 5, u64::from(bridge.0)]),
        instruction("NewArray", &[7, 0]),
        instruction("DefineOwnByIndex", &[7, 6, 0]),
        instruction("LoadConstUndefined", &[0]),
        instruction("GetArgumentsLength", &[8, 0]),
        instruction("LoadConstZero", &[9]),
        instruction("LoadConstUInt8", &[11, 1]),
    ];
    let test = code.len();
    code.push(instruction("JGreaterEqual", &[0, 9, 8]));
    let loop_start = code.len();
    code.extend([
        instruction("GetArgumentsPropByValStrict", &[12, 9, 0]),
        instruction("Add", &[10, 9, 11]),
        instruction("DefineOwnByVal", &[7, 12, 10, 1]),
        instruction("Add", &[9, 9, 11]),
        instruction("JLess", &[0, 9, 8]),
    ]);
    let loop_branch = code.len() - 1;
    let done = code.len();
    code.extend([
        instruction("Mov", &[15, 4]),
        instruction("Mov", &[14, 7]),
        instruction("Mov", &[13, 2]),
        instruction(
            "CallBuiltin",
            &[0, u64::from(crate::opcode::BUILTIN_APPLY), 4],
        ),
        instruction("Ret", &[0]),
    ]);
    let mut offset = 0;
    for inst in &mut code {
        inst.offset = offset;
        offset += inst.definition().size() as u32;
    }
    code[test].values[0] = u64::from(code[done].offset - code[test].offset);
    code[loop_branch].values[0] = u64::from(
        code[loop_start]
            .offset
            .wrapping_sub(code[loop_branch].offset),
    );
    // Generated branches use their long forms so their addresses cannot truncate.
    for index in [test, loop_branch] {
        let name = format!("{}Long", code[index].definition().name);
        code[index].opcode = code[index]
            .version
            .opcodes()
            .iter()
            .position(|o| o.name == name)
            .expect("long generated branch exists") as u8;
    }
    // Reassign positions after choosing the long branch layouts.
    let mut offset = 0;
    for inst in &mut code {
        inst.offset = offset;
        offset += inst.definition().size() as u32;
    }
    code[test].values[0] = u64::from(code[done].offset - code[test].offset);
    code[loop_branch].values[0] = u64::from(
        code[loop_start]
            .offset
            .wrapping_sub(code[loop_branch].offset),
    );
    let mut result = generated_function(
        function.name(),
        function.header.parameters,
        24,
        encode(&code),
    );
    result.header.read_cache = 1;
    result.header.flags = (function.header.flags & 4) | 1;
    Ok(result)
}
