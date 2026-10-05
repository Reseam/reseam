use std::cell::RefCell;

use boltffi::{data, export};
use reseam_hermes::{Argument, FunctionId, ModuleId};

use super::handles::{HandleSpace, bundle_path, with_ctx_result};

#[derive(Default)]
struct Functions {
    identities: HandleSpace,
    entries: Vec<FunctionHandle>,
}

#[derive(Clone)]
struct FunctionHandle {
    path: String,
    id: FunctionId,
}

thread_local! { static FUNCTIONS: RefCell<Functions> = RefCell::default(); }

pub(super) fn reset() {
    FUNCTIONS.with(|f| *f.borrow_mut() = Functions::default());
}

#[export]
pub fn hermes_version(path: String) -> Result<u32, String> {
    with_ctx_result(|ctx| ctx.hermes_version(&path).map_err(|e| e.to_string()))
}

#[export]
pub fn hermes_find_function(
    path: String,
    name: Option<String>,
    strings: Vec<String>,
    parameters: Option<u32>,
) -> Result<u32, String> {
    with_ctx_result(|ctx| {
        let strings: Vec<_> = strings.iter().map(String::as_str).collect();
        let id = ctx
            .hermes_find(&path, name.as_deref(), &strings, parameters)
            .map_err(|e| e.to_string())?;
        FUNCTIONS.with(|functions| {
            let mut functions = functions.borrow_mut();
            let slot = functions.entries.len();
            let handle = functions
                .identities
                .allocate(slot)
                .map_err(|e| e.to_string())?;
            functions.entries.push(FunctionHandle { path, id });
            Ok(handle)
        })
    })
}

/// A value bound into a wrap ahead of `original`.
#[data]
#[derive(Debug, Clone)]
pub enum HermesArgument {
    Bool(bool),
    Text(String),
    Export(HermesExportRef),
}

/// A callable export of a Hermes extension module.
#[data]
#[derive(Debug, Clone)]
pub struct HermesExportRef {
    pub module: String,
    pub name: String,
}

#[export]
pub fn hermes_wrap(
    handle: u32,
    module: String,
    export: String,
    bound: Vec<HermesArgument>,
) -> Result<(), String> {
    let function = FUNCTIONS.with(|functions| {
        let functions = functions.borrow();
        let slot = functions
            .identities
            .slot(handle)
            .ok_or("stale Hermes function handle")?;
        functions
            .entries
            .get(slot)
            .cloned()
            .ok_or("stale Hermes function handle")
    })?;
    with_ctx_result(|ctx| {
        let mut link = |name: &str| -> Result<ModuleId, String> {
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
            {
                return Err(
                    "Hermes module names contain only letters, digits, hyphens and underscores"
                        .into(),
                );
            }
            let version = ctx
                .hermes_version(&function.path)
                .map_err(|e| e.to_string())?;
            let path = bundle_path(&format!("resources/hermes/v{version}/{name}.hbc"));
            ctx.hermes_link(&function.path, &path)
                .map_err(|e| e.to_string())
        };
        let module = link(&module)?;
        let bound = bound
            .into_iter()
            .map(|argument| {
                Ok(match argument {
                    HermesArgument::Bool(value) => Argument::Bool(value),
                    HermesArgument::Text(text) => Argument::String(text),
                    HermesArgument::Export(export) => Argument::Export {
                        module: link(&export.module)?,
                        name: export.name,
                    },
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        ctx.hermes_wrap(&function.path, function.id, module, &export, &bound)
            .map_err(|e| e.to_string())
    })
}
