// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::cell::RefCell;
use std::path::PathBuf;

use boltffi::{data, export};
use reseam_hermes::{
    BlockId, Code, Condition, FunctionId, HookKind, Import, ModuleId, Operand, Place, SlotId,
    Statement,
};

use crate::bridge::handles::HandleSpace;
use crate::bridge::phase::{bundle_path, with_ctx_result};
use crate::context::{PatchContext, hermes_imports};

#[derive(Default)]
struct Functions {
    identities: HandleSpace,
    entries: Vec<FunctionId>,
}

thread_local! { static FUNCTIONS: RefCell<Functions> = RefCell::default(); }

pub(super) fn reset() {
    FUNCTIONS.with(|f| *f.borrow_mut() = Functions::default());
}

#[export]
pub fn hermes_version() -> Result<u32, String> {
    with_ctx_result(|ctx| ctx.hermes_version().map_err(|e| e.to_string()))
}

#[export]
pub fn hermes_find_function(
    name: Option<String>,
    strings: Vec<String>,
    parameters: Option<u32>,
) -> Result<u32, String> {
    with_ctx_result(|ctx| {
        let strings: Vec<_> = strings.iter().map(String::as_str).collect();
        let id = ctx
            .hermes_find(name.as_deref(), &strings, parameters)
            .map_err(|e| e.to_string())?;
        FUNCTIONS.with(|functions| {
            let mut functions = functions.borrow_mut();
            let slot = functions.entries.len();
            let handle = functions
                .identities
                .allocate(slot)
                .map_err(|e| e.to_string())?;
            functions.entries.push(id);
            Ok(handle)
        })
    })
}

fn function(handle: u32) -> Result<FunctionId, String> {
    FUNCTIONS.with(|functions| {
        let functions = functions.borrow();
        let slot = functions
            .identities
            .slot(handle)
            .ok_or("stale Hermes function handle")?;
        functions
            .entries
            .get(slot)
            .copied()
            .ok_or_else(|| "stale Hermes function handle".to_owned())
    })
}

/// A JavaScript module compiled to Hermes bytecode.
#[data]
#[derive(Debug, Clone)]
pub enum HermesModule {
    /// The app runtime's module, which the engine provides.
    AppRuntime,
    /// A JavaScript extension in the running patch's bundle, with every app module it imports.
    Bundle {
        name: String,
        imports: Vec<HermesImport>,
    },
}

/// An app module a JavaScript extension imports: its specifier, and the handle of an app function
/// the Metro module's factory creates, or of the factory itself.
#[data]
#[derive(Debug, Clone)]
pub struct HermesImport {
    pub specifier: String,
    pub module: u32,
}

/// The app modules the bundle's JavaScript extension `name` imports, by specifier.
#[export]
pub fn hermes_module_imports(name: String) -> Result<Vec<String>, String> {
    with_ctx_result(|ctx| hermes_imports(&extension_path(ctx, &name)?).map_err(|e| e.to_string()))
}

fn extension_path(ctx: &mut PatchContext<'_>, name: &str) -> Result<PathBuf, String> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err(
            "Hermes module names contain only letters, digits, hyphens and underscores".into(),
        );
    }
    let version = ctx.hermes_version().map_err(|e| e.to_string())?;
    Ok(bundle_path(&format!(
        "resources/hermes/v{version}/{name}.hbc"
    )))
}

/// Where a hook's code runs in an app function.
#[data]
#[derive(Debug, Clone, Copy)]
pub enum HermesHookKind {
    Before,
    After,
    Replace,
}

/// The code of a hook: blocks of statements, the first being the hook's body.
#[data]
#[derive(Debug, Clone)]
pub struct HermesCode {
    pub blocks: Vec<HermesBlock>,
}

#[data]
#[derive(Debug, Clone)]
pub struct HermesBlock {
    pub statements: Vec<HermesStatement>,
}

#[data]
#[derive(Debug, Clone)]
pub enum HermesStatement {
    Call {
        into: u32,
        module: HermesModule,
        export: String,
        arguments: Vec<HermesOperand>,
    },
    Assign {
        place: HermesPlace,
        value: HermesOperand,
    },
    Return {
        value: HermesOperand,
    },
    Branch {
        condition: HermesCondition,
        then: u32,
        otherwise: u32,
    },
}

#[data]
#[derive(Debug, Clone, Copy)]
pub enum HermesPlace {
    Param { index: u32 },
    Result,
    Slot { index: u32 },
}

#[data]
#[derive(Debug, Clone)]
pub enum HermesOperand {
    Receiver,
    Param { index: u32 },
    Result,
    Slot { index: u32 },
    Undefined,
    Null,
    Bool { value: bool },
    Int { value: i32 },
    Text { value: String },
}

#[data]
#[derive(Debug, Clone)]
pub enum HermesCondition {
    True {
        value: HermesOperand,
    },
    False {
        value: HermesOperand,
    },
    Null {
        value: HermesOperand,
    },
    NotNull {
        value: HermesOperand,
    },
    Equal {
        left: HermesOperand,
        right: HermesOperand,
    },
    NotEqual {
        left: HermesOperand,
        right: HermesOperand,
    },
}

#[export]
pub fn hermes_hook(handle: u32, kind: HermesHookKind, code: HermesCode) -> Result<(), String> {
    let target = function(handle)?;
    with_ctx_result(|ctx| {
        let code = Code {
            blocks: code
                .blocks
                .into_iter()
                .map(|block| {
                    block
                        .statements
                        .into_iter()
                        .map(|statement| self::statement(ctx, statement))
                        .collect()
                })
                .collect::<Result<_, String>>()?,
        };
        let kind = match kind {
            HermesHookKind::Before => HookKind::Before,
            HermesHookKind::After => HookKind::After,
            HermesHookKind::Replace => HookKind::Replace,
        };
        ctx.hermes_hook(target, kind, &code)
            .map_err(|e| e.to_string())
    })
}

fn statement(ctx: &mut PatchContext<'_>, statement: HermesStatement) -> Result<Statement, String> {
    Ok(match statement {
        HermesStatement::Call {
            into,
            module,
            export,
            arguments,
        } => Statement::Call {
            into: SlotId(into),
            module: link(ctx, &module)?,
            export,
            arguments: arguments.into_iter().map(operand).collect(),
        },
        HermesStatement::Assign { place, value } => Statement::Assign {
            place: match place {
                HermesPlace::Param { index } => Place::Param(index),
                HermesPlace::Result => Place::Result,
                HermesPlace::Slot { index } => Place::Slot(SlotId(index)),
            },
            value: operand(value),
        },
        HermesStatement::Return { value } => Statement::Return(operand(value)),
        HermesStatement::Branch {
            condition,
            then,
            otherwise,
        } => Statement::Branch {
            condition: match condition {
                HermesCondition::True { value } => Condition::True(operand(value)),
                HermesCondition::False { value } => Condition::False(operand(value)),
                HermesCondition::Null { value } => Condition::Null(operand(value)),
                HermesCondition::NotNull { value } => Condition::NotNull(operand(value)),
                HermesCondition::Equal { left, right } => {
                    Condition::Equal(operand(left), operand(right))
                }
                HermesCondition::NotEqual { left, right } => {
                    Condition::NotEqual(operand(left), operand(right))
                }
            },
            then: BlockId(then),
            otherwise: BlockId(otherwise),
        },
    })
}

fn operand(operand: HermesOperand) -> Operand {
    match operand {
        HermesOperand::Receiver => Operand::Receiver,
        HermesOperand::Param { index } => Operand::Param(index),
        HermesOperand::Result => Operand::Result,
        HermesOperand::Slot { index } => Operand::Slot(SlotId(index)),
        HermesOperand::Undefined => Operand::Undefined,
        HermesOperand::Null => Operand::Null,
        HermesOperand::Bool { value } => Operand::Bool(value),
        HermesOperand::Int { value } => Operand::Int(value),
        HermesOperand::Text { value } => Operand::Text(value),
    }
}

/// Links a module the first time a hook calls one of its exports.
fn link(ctx: &mut PatchContext<'_>, module: &HermesModule) -> Result<ModuleId, String> {
    match module {
        HermesModule::AppRuntime => ctx.hermes_link_app_runtime(),
        HermesModule::Bundle { name, imports } => {
            let path = extension_path(ctx, name)?;
            let imports = imports
                .iter()
                .map(|import| {
                    Ok(Import {
                        specifier: import.specifier.clone(),
                        function: function(import.module)?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            ctx.hermes_link(&path, &imports)
        }
    }
    .map_err(|e| e.to_string())
}
