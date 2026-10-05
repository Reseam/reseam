use std::collections::{BTreeMap, BTreeSet};

use crate::opcode::OperandKind;
use crate::{FunctionId, HermesFile, Result, StringId};

#[derive(Clone, Copy)]
enum Value {
    Object(usize),
    Function(FunctionId),
}

pub(crate) fn exports(file: &HermesFile<'_>) -> Result<Vec<StringId>> {
    resolve(file, file.global_function(), &mut BTreeSet::new())
        .map(|fields| fields.into_iter().collect())
}

fn resolve(
    file: &HermesFile<'_>,
    id: FunctionId,
    visiting: &mut BTreeSet<FunctionId>,
) -> Result<BTreeSet<StringId>> {
    if !visiting.insert(id) {
        return Ok(BTreeSet::new());
    }
    let mut registers = BTreeMap::<u32, Value>::new();
    let mut objects = Vec::<BTreeSet<StringId>>::new();
    let mut returned: Option<BTreeSet<StringId>> = None;
    let mut branched = false;
    for inst in file.function(id)?.instructions()? {
        let name = inst.definition().name;
        let v = &inst.values;
        let value = match name {
            "NewObject" => {
                let object = objects.len();
                objects.push(BTreeSet::new());
                Some(Value::Object(object))
            }
            "CreateClosure" | "CreateClosureLongIndex" => {
                Some(Value::Function(FunctionId(v[2] as u32)))
            }
            "Mov" | "MovLong" => registers.get(&(v[1] as u32)).copied(),
            "Call1" | "Call2" | "Call3" | "Call4" => {
                if let Some(Value::Function(callee)) = registers.get(&(v[1] as u32)) {
                    let fields = resolve(file, *callee, visiting)?;
                    let object = objects.len();
                    objects.push(fields);
                    Some(Value::Object(object))
                } else {
                    None
                }
            }
            _ => None,
        };
        if (name.starts_with("PutById") || matches!(name, "DefineOwnById" | "DefineOwnByIdLong"))
            && let Some(Value::Object(object)) = registers.get(&(v[0] as u32))
        {
            let key = StringId(v[3] as u32);
            if !branched && matches!(registers.get(&(v[1] as u32)), Some(Value::Function(_))) {
                objects[*object].insert(key);
            } else {
                objects[*object].remove(&key);
            }
        }
        if name == "Ret" {
            let fields = if let Some(Value::Object(object)) = registers.get(&(v[0] as u32)) {
                objects[*object].clone()
            } else {
                BTreeSet::new()
            };
            if let Some(returned) = &mut returned {
                returned.retain(|key| fields.contains(key));
            } else {
                returned = Some(fields);
            }
        }
        branched |= inst
            .definition()
            .operands
            .iter()
            .any(|o| matches!(o.kind, OperandKind::Addr8 | OperandKind::Addr32));
        let writes_first = inst.definition().writes_first_register();
        if writes_first {
            if let Some(value) = value {
                registers.insert(v[0] as u32, value);
            } else {
                registers.remove(&(v[0] as u32));
            }
        }
    }
    visiting.remove(&id);
    Ok(returned.unwrap_or_default())
}
