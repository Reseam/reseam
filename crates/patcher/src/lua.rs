use std::path::Path;

use mlua::prelude::*;

use crate::context::PatchContext;
use crate::error::{PatcherError, Result};
use crate::lua_insn;
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

        // SAFETY: The raw pointer is sound because the Lua VM is created and
        // destroyed within this function, and ctx outlives the VM.
        let ctx_ptr = ctx as *mut PatchContext<'_> as *mut PatchContext<'static>;
        let lua_ctx = lua.create_any_userdata(CtxPtr { ptr: ctx_ptr })?;
        let ctx_table = build_ctx_table(&lua, lua_ctx)?;

        execute_fn
            .call::<()>(ctx_table)
            .map_err(|e| PatcherError::PatchFailed {
                name: self.name.clone(),
                reason: e.to_string(),
            })?;

        Ok(())
    }
}

struct CtxPtr {
    ptr: *mut PatchContext<'static>,
}

// SAFETY: Single-threaded Lua VM, pointer valid for execute() duration.
unsafe impl Send for CtxPtr {}
unsafe impl Sync for CtxPtr {}

impl CtxPtr {
    fn r(&self) -> &PatchContext<'static> {
        unsafe { &*self.ptr }
    }
    fn w(&self) -> &mut PatchContext<'static> {
        unsafe { &mut *self.ptr }
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
    let stitch_table = lua.create_table()?;
    stitch_table.set(
        "log",
        lua.create_function(|_, msg: String| {
            eprintln!("[stitch:lua] {msg}");
            Ok(())
        })?,
    )?;
    globals.set("stitch", stitch_table)?;
    Ok(())
}

macro_rules! ctx_fn {
    ($lua:expr, $methods:expr, $name:literal, |$w:ident, $tbl:ident| $body:expr) => {
        $methods.set($name, $lua.create_function(|_, $tbl: LuaTable| {
            let ud: LuaAnyUserData = $tbl.get("_ud")?;
            let $w = ud.borrow::<CtxPtr>()?;
            $body
        })?)?;
    };
    ($lua:expr, $methods:expr, $name:literal, |$w:ident, $tbl:ident, $($arg:ident : $ty:ty),+| $body:expr) => {
        $methods.set($name, $lua.create_function(|_, ($tbl, $($arg),+): (LuaTable, $($ty),+)| {
            let ud: LuaAnyUserData = $tbl.get("_ud")?;
            let $w = ud.borrow::<CtxPtr>()?;
            $body
        })?)?;
    };
    ($lua:expr, $methods:expr, $name:literal, |$lua_name:ident, $w:ident, $tbl:ident| $body:expr) => {
        $methods.set($name, $lua.create_function(|$lua_name, $tbl: LuaTable| {
            let ud: LuaAnyUserData = $tbl.get("_ud")?;
            let $w = ud.borrow::<CtxPtr>()?;
            $body
        })?)?;
    };
    ($lua:expr, $methods:expr, $name:literal, |$lua_name:ident, $w:ident, $tbl:ident, $($arg:ident : $ty:ty),+| $body:expr) => {
        $methods.set($name, $lua.create_function(|$lua_name, ($tbl, $($arg),+): (LuaTable, $($ty),+)| {
            let ud: LuaAnyUserData = $tbl.get("_ud")?;
            let $w = ud.borrow::<CtxPtr>()?;
            $body
        })?)?;
    };
}

fn build_ctx_table(lua: &Lua, ud: LuaAnyUserData) -> Result<LuaTable> {
    let t = lua.create_table()?;
    t.set("_ud", ud)?;

    let m = lua.create_table()?;

    ctx_fn!(lua, m, "package_name", |w, tbl| {
        Ok(w.r().package_name().map(|s| s.to_owned()))
    });
    ctx_fn!(lua, m, "version_code", |w, tbl| {
        Ok(w.r().version_code())
    });
    ctx_fn!(lua, m, "version_name", |w, tbl| {
        Ok(w.r().version_name().map(|s| s.to_owned()))
    });
    ctx_fn!(lua, m, "min_sdk_version", |w, tbl| {
        Ok(w.r().manifest().min_sdk_version())
    });
    ctx_fn!(lua, m, "split_name", |w, tbl| {
        Ok(w.r().manifest().split_name().map(|s| s.to_owned()))
    });

    ctx_fn!(lua, m, "dex_count", |w, tbl| { Ok(w.r().dex_count()) });
    ctx_fn!(lua, m, "class_count", |w, tbl, dex_idx: usize| {
        match w.r().dex_file(dex_idx) {
            Some(dex) => Ok(dex.classes.len()),
            None => Ok(0),
        }
    });
    ctx_fn!(lua, m, "string", |w, tbl, dex_idx: usize, str_idx: u32| {
        match w.r().dex_file(dex_idx) {
            Some(dex) => Ok(dex
                .string(stitch_apk::stitch_dex::StringIdx(str_idx))
                .to_owned()),
            None => Err(LuaError::runtime(format!("invalid dex index: {dex_idx}"))),
        }
    });
    ctx_fn!(
        lua,
        m,
        "type_descriptor",
        |w, tbl, dex_idx: usize, type_idx: u32| {
            match w.r().dex_file(dex_idx) {
                Some(dex) => Ok(dex
                    .type_descriptor(stitch_apk::stitch_dex::TypeIdx(type_idx))
                    .to_owned()),
                None => Err(LuaError::runtime(format!("invalid dex index: {dex_idx}"))),
            }
        }
    );
    ctx_fn!(lua, m, "find_class", |lua, w, tbl, descriptor: String| {
        match w.r().find_class(&descriptor) {
            Some((dex_idx, class)) => {
                let r = lua.create_table()?;
                r.set("dex_index", dex_idx)?;
                r.set("class_type", class.class_type.0)?;
                r.set("access_flags", class.access_flags.bits())?;
                r.set("has_data", class.class_data.is_some())?;
                Ok(Some(r))
            }
            None => Ok(None),
        }
    });
    ctx_fn!(
        lua,
        m,
        "find_method",
        |lua, w, tbl, class_desc: String, method_name: String| {
            match w.r().find_method(&class_desc, &method_name) {
                Some((dex_idx, method)) => {
                    let r = lua.create_table()?;
                    r.set("dex_index", dex_idx)?;
                    r.set("method_idx", method.method.0)?;
                    r.set("access_flags", method.access_flags.bits())?;
                    r.set("has_code", method.code.is_some())?;
                    if let Some(code) = &method.code {
                        r.set("registers", code.registers_size)?;
                        r.set("ins", code.ins_size)?;
                        r.set("outs", code.outs_size)?;
                        r.set("insn_count", code.instructions.len())?;
                    }
                    Ok(Some(r))
                }
                None => Ok(None),
            }
        }
    );
    ctx_fn!(
        lua,
        m,
        "find_methods_with_opcodes",
        |lua, w, tbl, patterns: LuaTable| {
            let mut pats = Vec::new();
            for val in patterns.sequence_values::<String>() {
                pats.push(lua_insn::parse_pattern(&val?)?);
            }
            let results = w.r().find_methods_with_opcodes(&pats);
            let out = lua.create_table()?;
            for (i, (dex_idx, mm)) in results.iter().enumerate() {
                let r = lua.create_table()?;
                r.set("dex_index", *dex_idx)?;
                r.set("class_type", mm.class_idx.0)?;
                r.set("method_idx", mm.method_idx.0)?;
                r.set("access_flags", mm.method.access_flags.bits())?;
                out.set(i + 1, r)?;
            }
            Ok(out)
        }
    );

    ctx_fn!(
        lua,
        m,
        "intern_string",
        |w, tbl, dex_idx: usize, s: String| {
            match w.w().dex_file_mut(dex_idx) {
                Some(dex) => Ok(dex.intern_string(&s).0),
                None => Err(LuaError::runtime(format!("invalid dex index: {dex_idx}"))),
            }
        }
    );
    ctx_fn!(
        lua,
        m,
        "intern_type",
        |w, tbl, dex_idx: usize, desc: String| {
            match w.w().dex_file_mut(dex_idx) {
                Some(dex) => Ok(dex.intern_type(&desc).0),
                None => Err(LuaError::runtime(format!("invalid dex index: {dex_idx}"))),
            }
        }
    );
    ctx_fn!(
        lua,
        m,
        "intern_proto",
        |w, tbl, dex_idx: usize, desc: String| {
            match w.w().dex_file_mut(dex_idx) {
                Some(dex) => dex
                    .intern_proto(&desc)
                    .map(|p| p.0)
                    .map_err(|e| LuaError::runtime(e.to_string())),
                None => Err(LuaError::runtime(format!("invalid dex index: {dex_idx}"))),
            }
        }
    );
    ctx_fn!(
        lua,
        m,
        "intern_method",
        |w, tbl, dex_idx: usize, class: String, name: String, proto: String| {
            match w.w().dex_file_mut(dex_idx) {
                Some(dex) => dex
                    .intern_method(&class, &name, &proto)
                    .map(|m| m.0)
                    .map_err(|e| LuaError::runtime(e.to_string())),
                None => Err(LuaError::runtime(format!("invalid dex index: {dex_idx}"))),
            }
        }
    );
    ctx_fn!(
        lua,
        m,
        "intern_field",
        |w, tbl, dex_idx: usize, class: String, name: String, type_: String| {
            match w.w().dex_file_mut(dex_idx) {
                Some(dex) => dex
                    .intern_field(&class, &name, &type_)
                    .map(|f| f.0)
                    .map_err(|e| LuaError::runtime(e.to_string())),
                None => Err(LuaError::runtime(format!("invalid dex index: {dex_idx}"))),
            }
        }
    );
    ctx_fn!(lua, m, "build_lookups", |w, tbl, dex_idx: usize| {
        match w.w().dex_file_mut(dex_idx) {
            Some(dex) => {
                dex.build_lookups();
                Ok(())
            }
            None => Err(LuaError::runtime(format!("invalid dex index: {dex_idx}"))),
        }
    });
    ctx_fn!(
        lua,
        m,
        "remove_class",
        |w, tbl, dex_idx: usize, descriptor: String| {
            match w.w().dex_file_mut(dex_idx) {
                Some(dex) => {
                    let type_idx = match dex.find_type_idx(&descriptor) {
                        Some(idx) => idx,
                        None => return Ok(false),
                    };
                    Ok(dex.remove_class(type_idx).is_some())
                }
                None => Err(LuaError::runtime(format!("invalid dex index: {dex_idx}"))),
            }
        }
    );

    ctx_fn!(
        lua,
        m,
        "return_early",
        |w, tbl, class_desc: String, method_name: String| {
            match w.w().find_method_mut(&class_desc, &method_name) {
                Some((_, method)) => match method.code_mut() {
                    Some(code) => {
                        code.return_early();
                        Ok(true)
                    }
                    None => Ok(false),
                },
                None => Ok(false),
            }
        }
    );
    ctx_fn!(
        lua,
        m,
        "return_early_int",
        |w, tbl, class_desc: String, method_name: String, value: i32| {
            match w.w().find_method_mut(&class_desc, &method_name) {
                Some((_, method)) => match method.code_mut() {
                    Some(code) => {
                        code.return_early_int(value);
                        Ok(true)
                    }
                    None => Ok(false),
                },
                None => Ok(false),
            }
        }
    );

    ctx_fn!(
        lua,
        m,
        "get_instructions",
        |lua, w, tbl, class_desc: String, method_name: String| {
            match w.r().find_method(&class_desc, &method_name) {
                Some((_, method)) => match &method.code {
                    Some(code) => {
                        let out = lua.create_table()?;
                        for (i, insn) in code.instructions.iter().enumerate() {
                            out.set(i + 1, lua_insn::instruction_to_lua(lua, insn)?)?;
                        }
                        Ok(Some(out))
                    }
                    None => Ok(None),
                },
                None => Ok(None),
            }
        }
    );
    ctx_fn!(
        lua,
        m,
        "replace_instruction",
        |w, tbl, class_desc: String, method_name: String, index: usize, insn_tbl: LuaTable| {
            let insn = lua_insn::lua_to_instruction(&insn_tbl)?;
            match w.w().find_method_mut(&class_desc, &method_name) {
                Some((_, method)) => match method.code_mut() {
                    Some(code) => {
                        if index >= code.instructions.len() {
                            return Err(LuaError::runtime(format!(
                                "instruction index {index} out of bounds"
                            )));
                        }
                        code.replace_instruction(index, insn);
                        Ok(true)
                    }
                    None => Ok(false),
                },
                None => Ok(false),
            }
        }
    );
    ctx_fn!(
        lua,
        m,
        "insert_instruction",
        |w, tbl, class_desc: String, method_name: String, index: usize, insn_tbl: LuaTable| {
            let insn = lua_insn::lua_to_instruction(&insn_tbl)?;
            match w.w().find_method_mut(&class_desc, &method_name) {
                Some((_, method)) => match method.code_mut() {
                    Some(code) => {
                        if index > code.instructions.len() {
                            return Err(LuaError::runtime(format!(
                                "instruction index {index} out of bounds"
                            )));
                        }
                        code.insert_instruction(index, insn);
                        Ok(true)
                    }
                    None => Ok(false),
                },
                None => Ok(false),
            }
        }
    );
    ctx_fn!(
        lua,
        m,
        "remove_instruction",
        |w, tbl, class_desc: String, method_name: String, index: usize| {
            match w.w().find_method_mut(&class_desc, &method_name) {
                Some((_, method)) => match method.code_mut() {
                    Some(code) => {
                        if index >= code.instructions.len() {
                            return Err(LuaError::runtime(format!(
                                "instruction index {index} out of bounds"
                            )));
                        }
                        code.remove_instruction(index);
                        Ok(true)
                    }
                    None => Ok(false),
                },
                None => Ok(false),
            }
        }
    );
    ctx_fn!(
        lua,
        m,
        "set_instructions",
        |w, tbl, class_desc: String, method_name: String, insns_tbl: LuaTable| {
            let mut insns = Vec::new();
            for val in insns_tbl.sequence_values::<LuaTable>() {
                insns.push(lua_insn::lua_to_instruction(&val?)?);
            }
            match w.w().find_method_mut(&class_desc, &method_name) {
                Some((_, method)) => match method.code_mut() {
                    Some(code) => {
                        code.set_instructions(insns);
                        Ok(true)
                    }
                    None => Ok(false),
                },
                None => Ok(false),
            }
        }
    );

    ctx_fn!(lua, m, "set_version_code", |w, tbl, code: u32| {
        w.w().manifest_mut().set_version_code(code);
        Ok(())
    });
    ctx_fn!(lua, m, "set_version_name", |w, tbl, name: String| {
        w.w().manifest_mut().set_version_name(&name);
        Ok(())
    });
    ctx_fn!(lua, m, "set_min_sdk", |w, tbl, sdk: u32| {
        w.w().manifest_mut().set_min_sdk(sdk);
        Ok(())
    });
    ctx_fn!(lua, m, "add_permission", |w, tbl, perm: String| {
        w.w().manifest_mut().add_permission(&perm);
        Ok(())
    });
    ctx_fn!(
        lua,
        m,
        "set_attribute_int",
        |w, tbl, element: String, res_id: u32, value: i32| {
            w.w()
                .manifest_mut()
                .set_attribute_int(&element, res_id, value);
            Ok(())
        }
    );
    ctx_fn!(
        lua,
        m,
        "set_attribute_string",
        |w, tbl, element: String, res_id: u32, value: String| {
            w.w()
                .manifest_mut()
                .set_attribute_string(&element, res_id, &value);
            Ok(())
        }
    );

    ctx_fn!(lua, m, "has_resources", |w, tbl| {
        Ok(w.r().resources().is_some())
    });
    ctx_fn!(lua, m, "resource_string", |w, tbl, index: u32| {
        match w.r().resources() {
            Some(res) => Ok(res.get_string(index).map(|s| s.to_owned())),
            None => Err(LuaError::runtime("no resources.arsc")),
        }
    });
    ctx_fn!(
        lua,
        m,
        "set_resource_string",
        |w, tbl, index: u32, value: String| {
            match w.w().resources_mut() {
                Some(res) => {
                    res.set_string(index, value);
                    Ok(())
                }
                None => Err(LuaError::runtime("no resources.arsc")),
            }
        }
    );
    ctx_fn!(
        lua,
        m,
        "find_resource_entries_by_string",
        |lua, w, tbl, string_index: u32| {
            match w.r().resources() {
                Some(res) => {
                    let refs = res.find_entries_by_string(string_index);
                    let out = lua.create_table()?;
                    for (i, r) in refs.iter().enumerate() {
                        let e = lua.create_table()?;
                        e.set("res_id", r.res_id)?;
                        e.set("package_id", r.package_id)?;
                        e.set("type_id", r.type_id)?;
                        e.set("entry_index", r.entry_index)?;
                        e.set("key_name", r.key_name.clone())?;
                        out.set(i + 1, e)?;
                    }
                    Ok(out)
                }
                None => Err(LuaError::runtime("no resources.arsc")),
            }
        }
    );
    ctx_fn!(
        lua,
        m,
        "replace_resource_entry_string",
        |w, tbl, res_id: u32, new_string_index: u32| {
            match w.w().resources_mut() {
                Some(res) => {
                    res.replace_entry_string(res_id, new_string_index);
                    Ok(())
                }
                None => Err(LuaError::runtime("no resources.arsc")),
            }
        }
    );

    ctx_fn!(
        lua,
        m,
        "merge_extension_dex",
        |w, tbl, paths: Vec<String>| {
            w.w()
                .merge_extension_dex(&paths)
                .map_err(|e| LuaError::runtime(e.to_string()))
        }
    );

    let meta = lua.create_table()?;
    meta.set("__index", m)?;
    t.set_metatable(Some(meta));

    Ok(t)
}
