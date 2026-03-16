use std::path::Path;

use mlua::prelude::*;

use crate::context::PatchContext;
use crate::error::{PatcherError, Result};
use crate::patch::Patch;

struct LuaPatch {
    name: String,
    description: String,
    compatible_packages: Vec<String>,
    compatible_versions: Vec<String>,
    enabled_by_default: bool,
    source: String,
    script_path: String,
}

impl Patch for LuaPatch {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn compatible_packages(&self) -> &[String] {
        &self.compatible_packages
    }

    fn compatible_versions(&self) -> &[String] {
        &self.compatible_versions
    }

    fn enabled_by_default(&self) -> bool {
        self.enabled_by_default
    }

    fn execute(&self, ctx: &mut PatchContext) -> Result<()> {
        let lua = Lua::new();
        register_api(&lua)?;

        let patch_table: LuaTable = lua
            .load(&self.source)
            .set_name(&self.script_path)
            .eval()
            .map_err(|e| PatcherError::PatchFailed {
                name: self.name.clone(),
                reason: e.to_string(),
            })?;

        let execute_fn: LuaFunction =
            patch_table
                .get("execute")
                .map_err(|e| PatcherError::PatchFailed {
                    name: self.name.clone(),
                    reason: format!("missing execute function: {e}"),
                })?;

        // SAFETY: The raw pointer's 'static cast is sound because the Lua VM
        // is created and destroyed within this function, and ctx outlives it.
        let ctx_ptr = ctx as *mut PatchContext<'_> as *mut PatchContext<'static>;
        let lua_ctx = lua.create_any_userdata(LuaCtxWrapper { ctx: ctx_ptr })?;

        register_ctx_methods(&lua, &lua_ctx)?;

        execute_fn
            .call::<()>(lua_ctx)
            .map_err(|e| PatcherError::PatchFailed {
                name: self.name.clone(),
                reason: e.to_string(),
            })?;

        Ok(())
    }
}

struct LuaCtxWrapper {
    ctx: *mut PatchContext<'static>,
}

// SAFETY: Only used within a single-threaded Lua VM whose lifetime is
// bounded by a single execute() call. The pointer remains valid throughout.
unsafe impl Send for LuaCtxWrapper {}
unsafe impl Sync for LuaCtxWrapper {}

impl LuaCtxWrapper {
    fn ctx(&self) -> &PatchContext<'static> {
        unsafe { &*self.ctx }
    }

    fn ctx_mut(&self) -> &mut PatchContext<'static> {
        unsafe { &mut *self.ctx }
    }
}

pub fn load_lua_patch(path: impl AsRef<Path>) -> Result<Box<dyn Patch>> {
    let path = path.as_ref();
    let source = std::fs::read_to_string(path)?;
    let script_path = path.display().to_string();

    let lua = Lua::new();
    let table: LuaTable = lua
        .load(&source)
        .set_name(&script_path)
        .eval()
        .map_err(|e| PatcherError::Bundle {
            reason: format!("failed to load {script_path}: {e}"),
        })?;

    let name: String = table.get("name").map_err(|e| PatcherError::Bundle {
        reason: format!("{script_path}: missing 'name': {e}"),
    })?;

    let description: String = table.get("description").unwrap_or_default();

    let compatible_packages: Vec<String> = table
        .get::<Option<Vec<String>>>("compatible_packages")
        .unwrap_or(None)
        .unwrap_or_default();

    let compatible_versions: Vec<String> = table
        .get::<Option<Vec<String>>>("compatible_versions")
        .unwrap_or(None)
        .unwrap_or_default();

    let enabled_by_default: bool = table.get("enabled_by_default").unwrap_or(true);

    let _: LuaFunction = table.get("execute").map_err(|e| PatcherError::Bundle {
        reason: format!("{script_path}: missing 'execute': {e}"),
    })?;

    Ok(Box::new(LuaPatch {
        name,
        description,
        compatible_packages,
        compatible_versions,
        enabled_by_default,
        source,
        script_path,
    }))
}

fn register_api(lua: &Lua) -> Result<()> {
    let globals = lua.globals();

    let log_fn = lua.create_function(|_, msg: String| {
        eprintln!("[stitch:lua] {msg}");
        Ok(())
    })?;

    let stitch_table = lua.create_table()?;
    stitch_table.set("log", log_fn)?;
    globals.set("stitch", stitch_table)?;

    Ok(())
}

fn register_ctx_methods(lua: &Lua, ctx_ud: &LuaAnyUserData) -> Result<()> {
    let methods = lua.create_table()?;

    methods.set(
        "package_name",
        lua.create_function(|_, ud: LuaAnyUserData| {
            let wrapper = ud.borrow::<LuaCtxWrapper>()?;
            Ok(wrapper.ctx().package_name().map(|s| s.to_owned()))
        })?,
    )?;

    methods.set(
        "version_code",
        lua.create_function(|_, ud: LuaAnyUserData| {
            let wrapper = ud.borrow::<LuaCtxWrapper>()?;
            Ok(wrapper.ctx().version_code())
        })?,
    )?;

    methods.set(
        "version_name",
        lua.create_function(|_, ud: LuaAnyUserData| {
            let wrapper = ud.borrow::<LuaCtxWrapper>()?;
            Ok(wrapper.ctx().version_name().map(|s| s.to_owned()))
        })?,
    )?;

    methods.set(
        "dex_count",
        lua.create_function(|_, ud: LuaAnyUserData| {
            let wrapper = ud.borrow::<LuaCtxWrapper>()?;
            Ok(wrapper.ctx().dex_count())
        })?,
    )?;

    methods.set(
        "find_class",
        lua.create_function(|lua, (ud, descriptor): (LuaAnyUserData, String)| {
            let wrapper = ud.borrow::<LuaCtxWrapper>()?;
            match wrapper.ctx().find_class(&descriptor) {
                Some((dex_idx, class)) => {
                    let t = lua.create_table()?;
                    t.set("dex_index", dex_idx)?;
                    t.set("class_type", class.class_type.0)?;
                    t.set("access_flags", class.access_flags.bits())?;
                    t.set("has_data", class.class_data.is_some())?;
                    Ok(Some(t))
                }
                None => Ok(None),
            }
        })?,
    )?;

    methods.set(
        "find_method",
        lua.create_function(
            |lua, (ud, class_desc, method_name): (LuaAnyUserData, String, String)| {
                let wrapper = ud.borrow::<LuaCtxWrapper>()?;
                match wrapper.ctx().find_method(&class_desc, &method_name) {
                    Some((dex_idx, method)) => {
                        let t = lua.create_table()?;
                        t.set("dex_index", dex_idx)?;
                        t.set("method_idx", method.method.0)?;
                        t.set("access_flags", method.access_flags.bits())?;
                        t.set("has_code", method.code.is_some())?;
                        if let Some(code) = &method.code {
                            t.set("registers", code.registers_size)?;
                            t.set("ins", code.ins_size)?;
                            t.set("outs", code.outs_size)?;
                            t.set("insn_count", code.instructions.len())?;
                        }
                        Ok(Some(t))
                    }
                    None => Ok(None),
                }
            },
        )?,
    )?;

    methods.set(
        "return_early",
        lua.create_function(
            |_, (ud, class_desc, method_name): (LuaAnyUserData, String, String)| {
                let wrapper = ud.borrow::<LuaCtxWrapper>()?;
                match wrapper.ctx_mut().find_method_mut(&class_desc, &method_name) {
                    Some((_, method)) => match method.code_mut() {
                        Some(code) => {
                            code.return_early();
                            Ok(true)
                        }
                        None => Ok(false),
                    },
                    None => Ok(false),
                }
            },
        )?,
    )?;

    methods.set(
        "return_early_int",
        lua.create_function(
            |_,
             (ud, class_desc, method_name, value): (LuaAnyUserData, String, String, i32)| {
                let wrapper = ud.borrow::<LuaCtxWrapper>()?;
                match wrapper.ctx_mut().find_method_mut(&class_desc, &method_name) {
                    Some((_, method)) => match method.code_mut() {
                        Some(code) => {
                            code.return_early_int(value);
                            Ok(true)
                        }
                        None => Ok(false),
                    },
                    None => Ok(false),
                }
            },
        )?,
    )?;

    methods.set(
        "class_count",
        lua.create_function(|_, (ud, dex_idx): (LuaAnyUserData, usize)| {
            let wrapper = ud.borrow::<LuaCtxWrapper>()?;
            match wrapper.ctx().dex_file(dex_idx) {
                Some(dex) => Ok(dex.classes.len()),
                None => Ok(0),
            }
        })?,
    )?;

    methods.set(
        "string",
        lua.create_function(|_, (ud, dex_idx, str_idx): (LuaAnyUserData, usize, u32)| {
            let wrapper = ud.borrow::<LuaCtxWrapper>()?;
            match wrapper.ctx().dex_file(dex_idx) {
                Some(dex) => {
                    let s = dex.string(stitch_apk::stitch_dex::StringIdx(str_idx));
                    Ok(s.to_owned())
                }
                None => Err(LuaError::runtime(format!("invalid dex index: {dex_idx}"))),
            }
        })?,
    )?;

    methods.set(
        "type_descriptor",
        lua.create_function(
            |_, (ud, dex_idx, type_idx): (LuaAnyUserData, usize, u32)| {
                let wrapper = ud.borrow::<LuaCtxWrapper>()?;
                match wrapper.ctx().dex_file(dex_idx) {
                    Some(dex) => {
                        let s = dex.type_descriptor(stitch_apk::stitch_dex::TypeIdx(type_idx));
                        Ok(s.to_owned())
                    }
                    None => Err(LuaError::runtime(format!("invalid dex index: {dex_idx}"))),
                }
            },
        )?,
    )?;

    let meta = lua.create_table()?;
    meta.set("__index", methods)?;

    lua.globals().set("__stitch_ctx", ctx_ud.clone())?;
    lua.globals().set("__stitch_meta", meta)?;
    lua.load(
        "debug.setmetatable(__stitch_ctx, __stitch_meta); __stitch_ctx = nil; __stitch_meta = nil",
    )
    .exec()?;

    Ok(())
}
