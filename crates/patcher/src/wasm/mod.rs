mod bytecode;
pub(crate) mod convert;
mod log_host;
mod manifest;
mod options;
mod resources;
mod xml;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{IoView, WasiCtxBuilder};

use stitch_apk::axml::reader::AxmlDocument;

use crate::context::PatchContext;
use crate::error::PatcherError;
use crate::patch::{Compatibility, Patch};

wasmtime::component::bindgen!({
    path: "wit/stitch-patch.wit",
    world: "stitch-patch",
    async: false,
});

impl stitch::patch::types::Host for WasmState {}

// ── Handle table ──

#[derive(Clone, Copy)]
struct MethodHandle {
    dex_idx: usize,
    class_idx: usize,
    method_idx: usize,
    is_virtual: bool,
}

#[derive(Clone, Copy)]
struct ClassHandle {
    dex_idx: usize,
    class_idx: usize,
}

#[derive(Default)]
struct HandleTable {
    methods: Vec<MethodHandle>,
    classes: Vec<ClassHandle>,
    method_lookup: HashMap<(usize, usize, usize, bool), u32>,
    class_lookup: HashMap<(usize, usize), u32>,
}

impl HandleTable {
    fn alloc_method(&mut self, dex_idx: usize, class_idx: usize, method_idx: usize, is_virtual: bool) -> u32 {
        let key = (dex_idx, class_idx, method_idx, is_virtual);
        if let Some(&h) = self.method_lookup.get(&key) {
            return h;
        }
        let h = self.methods.len() as u32;
        self.methods.push(MethodHandle { dex_idx, class_idx, method_idx, is_virtual });
        self.method_lookup.insert(key, h);
        h
    }

    fn get_method(&self, handle: u32) -> Option<MethodHandle> {
        self.methods.get(handle as usize).copied()
    }

    fn alloc_class(&mut self, dex_idx: usize, class_idx: usize) -> u32 {
        let key = (dex_idx, class_idx);
        if let Some(&h) = self.class_lookup.get(&key) {
            return h;
        }
        let h = self.classes.len() as u32;
        self.classes.push(ClassHandle { dex_idx, class_idx });
        self.class_lookup.insert(key, h);
        h
    }

    fn get_class(&self, handle: u32) -> Option<ClassHandle> {
        self.classes.get(handle as usize).copied()
    }
}

// ── WASM state ──

struct WasmState {
    wasi: wasmtime_wasi::WasiCtx,
    table: wasmtime::component::ResourceTable,
    ctx_ptr: *mut (),
    handles: HandleTable,
    xml_documents: Vec<Option<(AxmlDocument, String)>>,
    pending_elements: Vec<xml::PendingElement>,
    bundle_dir: Option<PathBuf>,
}

// SAFETY: WasmState is only used single-threaded within a single Store.
unsafe impl Send for WasmState {}

impl WasmState {
    fn ctx(&self) -> &mut PatchContext<'_> {
        unsafe { &mut *(self.ctx_ptr as *mut PatchContext<'_>) }
    }
}

impl IoView for WasmState {
    fn table(&mut self) -> &mut wasmtime::component::ResourceTable {
        &mut self.table
    }
}

impl wasmtime_wasi::WasiView for WasmState {
    fn ctx(&mut self) -> &mut wasmtime_wasi::WasiCtx {
        &mut self.wasi
    }
}

// ── Helpers ──

fn wasm_err(msg: impl std::fmt::Display) -> PatcherError {
    PatcherError::Wasm { reason: msg.to_string() }
}

use stitch_apk::stitch_dex::{DexFile, EncodedMethod};

fn try_get_method_ref(dex: &DexFile, mh: MethodHandle) -> Option<&EncodedMethod> {
    let class = dex.classes.get(mh.class_idx)?;
    let data = class.class_data.as_ref()?;
    if mh.is_virtual {
        data.virtual_methods.get(mh.method_idx)
    } else {
        data.direct_methods.get(mh.method_idx)
    }
}

fn try_get_method_mut(dex: &mut DexFile, mh: MethodHandle) -> Option<&mut EncodedMethod> {
    let class = dex.classes.get_mut(mh.class_idx)?;
    let data = class.class_data.as_mut()?;
    if mh.is_virtual {
        data.virtual_methods.get_mut(mh.method_idx)
    } else {
        data.direct_methods.get_mut(mh.method_idx)
    }
}

fn get_method_ref(dex: &DexFile, mh: MethodHandle) -> &EncodedMethod {
    try_get_method_ref(dex, mh).expect("invalid method handle: class has no data or index out of bounds")
}

fn get_method_mut(dex: &mut DexFile, mh: MethodHandle) -> &mut EncodedMethod {
    try_get_method_mut(dex, mh).expect("invalid method handle: class has no data or index out of bounds")
}

fn find_method_location(ctx: &PatchContext<'_>, dex_idx: usize, method: &EncodedMethod) -> Option<(usize, usize, bool)> {
    let dex = ctx.dex_file(dex_idx)?;
    for (ci, class) in dex.classes.iter().enumerate() {
        if let Some(data) = &class.class_data {
            for (mi, m) in data.direct_methods.iter().enumerate() {
                if std::ptr::eq(m, method) {
                    return Some((ci, mi, false));
                }
            }
            for (mi, m) in data.virtual_methods.iter().enumerate() {
                if std::ptr::eq(m, method) {
                    return Some((ci, mi, true));
                }
            }
        }
    }
    None
}

fn method_match_location(ctx: &PatchContext<'_>, dex_idx: usize, mm: &stitch_apk::stitch_dex::MethodMatch<'_>) -> Option<(usize, usize, bool)> {
    find_method_location(ctx, dex_idx, mm.method)
}

// ── Engine / Store ──

fn create_engine() -> crate::error::Result<Engine> {
    let mut config = Config::new();
    config.wasm_component_model(true);
    Engine::new(&config).map_err(|e| wasm_err(format!("failed to create WASM engine: {e}")))
}

fn create_store(engine: &Engine, ctx_ptr: *mut (), bundle_dir: Option<PathBuf>) -> Store<WasmState> {
    let wasi = WasiCtxBuilder::new().build();
    let state = WasmState {
        wasi,
        table: wasmtime::component::ResourceTable::new(),
        ctx_ptr,
        handles: HandleTable::default(),
        xml_documents: Vec::new(),
        pending_elements: Vec::new(),
        bundle_dir,
    };
    Store::new(engine, state)
}

// ── WasmPatch ──

pub struct WasmPatch {
    name: String,
    description: String,
    compatible_with: Vec<Compatibility>,
    enabled_by_default: bool,
    depends_on: Vec<&'static str>,
    options: Vec<crate::options::OptionDeclaration>,
    engine: Engine,
    component: Component,
    path: PathBuf,
}

impl std::fmt::Debug for WasmPatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmPatch")
            .field("name", &self.name)
            .field("path", &self.path)
            .finish()
    }
}

fn convert_option_type(ot: &stitch::patch::types::OptionType) -> crate::options::OptionType {
    use stitch::patch::types::OptionType as W;
    match ot {
        W::StringType => crate::options::OptionType::String,
        W::BoolType => crate::options::OptionType::Bool,
        W::IntType => crate::options::OptionType::Int,
        W::FloatType => crate::options::OptionType::Float,
        W::StringListType => crate::options::OptionType::StringList,
        W::PathType => crate::options::OptionType::Path,
    }
}

pub fn load_wasm_patch(path: impl AsRef<Path>) -> crate::error::Result<Box<dyn Patch>> {
    let path = path.as_ref();
    let wasm_bytes = std::fs::read(path).map_err(|e| PatcherError::Wasm {
        reason: format!("failed to read {}: {e}", path.display()),
    })?;

    let engine = create_engine()?;
    let component = Component::new(&engine, &wasm_bytes).map_err(|e| PatcherError::Wasm {
        reason: format!("failed to compile {}: {e}", path.display()),
    })?;

    let mut linker: Linker<WasmState> = Linker::new(&engine);
    wasmtime_wasi::add_to_linker_sync(&mut linker).map_err(|e| wasm_err(format!("failed to link WASI: {e}")))?;
    StitchPatch::add_to_linker(&mut linker, |s| s).map_err(|e| wasm_err(format!("failed to add bindings: {e}")))?;

    let mut store = create_store(&engine, std::ptr::null_mut(), None);
    let patch_instance = StitchPatch::instantiate(&mut store, &component, &linker)
        .map_err(|e| PatcherError::Wasm {
            reason: format!("failed to instantiate {}: {e}", path.display()),
        })?;

    let metadata = patch_instance.call_metadata(&mut store)
        .map_err(|e| PatcherError::Wasm {
            reason: format!("{}: metadata() failed: {e}", path.display()),
        })?;

    let options_decls = patch_instance.call_declare_options(&mut store)
        .map_err(|e| PatcherError::Wasm {
            reason: format!("{}: declare_options() failed: {e}", path.display()),
        })?;

    let options: Vec<crate::options::OptionDeclaration> = options_decls.into_iter().map(|od| {
        crate::options::OptionDeclaration {
            key: od.key,
            title: od.title,
            description: od.description,
            option_type: convert_option_type(&od.option_type),
            default_value: od.default_value.map(crate::options::OptionValue::String),
            valid_values: od.valid_values,
            required: od.required,
        }
    }).collect();

    let compatible_with: Vec<Compatibility> = metadata.compatible_with.into_iter().map(|c| {
        Compatibility {
            package: c.package,
            versions: c.versions,
        }
    }).collect();

    let depends_on: Vec<&'static str> = metadata.depends_on
        .into_iter()
        .map(|s| &*Box::leak(s.into_boxed_str()))
        .collect();

    Ok(Box::new(WasmPatch {
        name: metadata.name,
        description: metadata.description,
        compatible_with,
        enabled_by_default: metadata.enabled_by_default,
        depends_on,
        options,
        engine,
        component,
        path: path.to_path_buf(),
    }))
}

impl Patch for WasmPatch {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn compatible_with(&self) -> &[Compatibility] {
        &self.compatible_with
    }

    fn enabled_by_default(&self) -> bool {
        self.enabled_by_default
    }

    fn depends_on(&self) -> &[&str] {
        &self.depends_on
    }

    fn options(&self) -> &[crate::options::OptionDeclaration] {
        &self.options
    }

    fn execute(&self, ctx: &mut PatchContext) -> crate::error::Result<()> {
        let mut linker: Linker<WasmState> = Linker::new(&self.engine);
        wasmtime_wasi::add_to_linker_sync(&mut linker)
            .map_err(|e| wasm_err(format!("failed to link WASI: {e}")))?;
        StitchPatch::add_to_linker(&mut linker, |s| s)
            .map_err(|e| wasm_err(format!("failed to add bindings: {e}")))?;

        let bundle_dir = self.path.parent().map(|p| p.to_path_buf());
        let ctx_ptr = ctx as *mut PatchContext as *mut ();
        let mut store = create_store(&self.engine, ctx_ptr, bundle_dir);

        let patch_instance = StitchPatch::instantiate(&mut store, &self.component, &linker)
            .map_err(|e| wasm_err(format!("failed to instantiate {}: {e}", self.path.display())))?;

        match patch_instance.call_execute(&mut store) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(msg)) => Err(PatcherError::Wasm {
                reason: format!("{}: {msg}", self.path.display()),
            }),
            Err(e) => Err(PatcherError::Wasm {
                reason: format!("{}: execute() trapped: {e}", self.path.display()),
            }),
        }
    }
}
