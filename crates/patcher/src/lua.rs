use std::path::Path;

use mlua::prelude::*;

use crate::context::PatchContext;
use crate::error::{PatcherError, Result};
use crate::lua_insn;
use crate::patch::{Compatibility, Patch};

struct LuaPatch {
    name: String,
    description: String,
    compatible_with: Vec<Compatibility>,
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
    fn compatible_with(&self) -> &[Compatibility] {
        &self.compatible_with
    }
    fn enabled_by_default(&self) -> bool {
        self.enabled_by_default
    }

    fn execute(&self, ctx: &mut PatchContext) -> Result<()> {
        let lua = Lua::new();

        // SAFETY: The raw pointer is sound because the Lua VM is created and
        // destroyed within this function, and ctx outlives the VM.
        let ctx_ptr = ctx as *mut PatchContext<'_> as *mut PatchContext<'static>;
        let lua_ctx = lua.create_any_userdata(CtxPtr { ptr: ctx_ptr })?;

        register_api(&lua, &lua_ctx)?;

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

    let compatible_with = parse_lua_compat(&table)?;

    let enabled_by_default: bool = table
        .get::<Option<bool>>("enabled_by_default")
        .unwrap_or(None)
        .unwrap_or(true);
    let _: LuaFunction = table.get("execute").map_err(|e| PatcherError::Bundle {
        reason: format!("{script_path}: missing 'execute': {e}"),
    })?;

    Ok(Box::new(LuaPatch {
        name,
        description,
        compatible_with,
        enabled_by_default,
        source,
        script_path,
    }))
}

fn parse_lua_compat(table: &LuaTable) -> Result<Vec<Compatibility>> {
    let compat: Option<LuaTable> = table.get("compatible_with").unwrap_or(None);
    let Some(compat) = compat else {
        return Ok(Vec::new());
    };

    let mut result = Vec::new();
    for val in compat.sequence_values::<LuaValue>() {
        let val = val?;
        match val {
            LuaValue::String(s) => {
                result.push(Compatibility::package(s.to_str()?.to_owned()));
            }
            LuaValue::Table(t) => {
                let package: String = t.get(1)?;
                let versions: Vec<String> = t.get::<Option<Vec<String>>>(2)?.unwrap_or_default();
                result.push(Compatibility::with_versions(package, versions));
            }
            _ => {
                return Err(PatcherError::Bundle {
                    reason: "compatible_with entries must be strings or {package, {versions}}".into(),
                });
            }
        }
    }
    Ok(result)
}

fn register_api(lua: &Lua, ctx_ud: &LuaAnyUserData) -> Result<()> {
    let globals = lua.globals();
    let stitch_table = lua.create_table()?;
    let ud_clone = ctx_ud.clone();
    stitch_table.set(
        "log",
        lua.create_function(move |_, msg: String| {
            let w = ud_clone.borrow::<CtxPtr>()?;
            w.w().log().info(msg);
            Ok(())
        })?,
    )?;
    globals.set("stitch", stitch_table)?;
    Ok(())
}

// ─── Handle types ───────────────────────────────────────────────────────────

struct MethodRef {
    class_descriptor: String,
    method_name: String,
    dex_index: usize,
}

struct ClassRef {
    descriptor: String,
    dex_index: usize,
}

struct DexRef {
    index: usize,
}

macro_rules! with_ctx {
    ($tbl:expr, |$w:ident| $body:expr) => {{
        let ud: LuaAnyUserData = $tbl.get("_ud")?;
        let $w = ud.borrow::<CtxPtr>()?;
        $body
    }};
}

macro_rules! with_method {
    ($tbl:expr, |$w:ident, $mref:ident| $body:expr) => {{
        let ud: LuaAnyUserData = $tbl.get("_ud")?;
        let $w = ud.borrow::<CtxPtr>()?;
        let $mref: &MethodRef = &*$tbl.get::<LuaAnyUserData>("_ref")?.borrow::<MethodRef>()?;
        $body
    }};
}

macro_rules! with_dex {
    ($tbl:expr, |$w:ident, $dref:ident| $body:expr) => {{
        let ud: LuaAnyUserData = $tbl.get("_ud")?;
        let $w = ud.borrow::<CtxPtr>()?;
        let $dref: &DexRef = &*$tbl.get::<LuaAnyUserData>("_dref")?.borrow::<DexRef>()?;
        $body
    }};
}

fn build_method_table(lua: &Lua, ud: &LuaAnyUserData, mref: MethodRef, mm_info: Option<MethodMatchInfo>) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set("_ud", ud.clone())?;

    t.set("dex_index", mref.dex_index)?;
    t.set("class_descriptor", mref.class_descriptor.as_str())?;
    t.set("method_name", mref.method_name.as_str())?;

    if let Some(info) = mm_info {
        t.set("class_type", info.class_type)?;
        t.set("method_idx", info.method_idx)?;
        t.set("access_flags", info.access_flags)?;
    }

    let dex_table = build_dex_table(lua, ud, DexRef { index: mref.dex_index })?;
    t.set("dex", dex_table)?;

    t.set("_ref", lua.create_any_userdata(mref)?)?;

    let meta = lua.create_table()?;
    let index_table = lua.create_table()?;

    index_table.set("return_early", lua.create_function(|_, tbl: LuaTable| {
        with_method!(tbl, |w, mref| {
            match w.w().find_method_mut(&mref.class_descriptor, &mref.method_name) {
                Some((_, method)) => {
                    method.return_early();
                    Ok(true)
                }
                None => Ok(false),
            }
        })
    })?)?;

    index_table.set("return_early_int", lua.create_function(|_, (tbl, value): (LuaTable, i32)| {
        with_method!(tbl, |w, mref| {
            match w.w().find_method_mut(&mref.class_descriptor, &mref.method_name) {
                Some((_, method)) => {
                    method.return_early_int(value);
                    Ok(true)
                }
                None => Ok(false),
            }
        })
    })?)?;

    index_table.set("instructions", lua.create_function(|lua, tbl: LuaTable| {
        with_method!(tbl, |w, mref| {
            match w.r().find_method(&mref.class_descriptor, &mref.method_name) {
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
        })
    })?)?;

    index_table.set("set_instructions", lua.create_function(|_, (tbl, insns_tbl): (LuaTable, LuaTable)| {
        let mut insns = Vec::new();
        for val in insns_tbl.sequence_values::<LuaTable>() {
            insns.push(lua_insn::lua_to_instruction(&val?)?);
        }
        with_method!(tbl, |w, mref| {
            match w.w().find_method_mut(&mref.class_descriptor, &mref.method_name) {
                Some((_, method)) => match method.code_mut() {
                    Some(code) => {
                        code.set_instructions(insns);
                        Ok(true)
                    }
                    None => Ok(false),
                },
                None => Ok(false),
            }
        })
    })?)?;

    index_table.set("replace_instruction", lua.create_function(|_, (tbl, index, insn_tbl): (LuaTable, usize, LuaTable)| {
        let insn = lua_insn::lua_to_instruction(&insn_tbl)?;
        with_method!(tbl, |w, mref| {
            match w.w().find_method_mut(&mref.class_descriptor, &mref.method_name) {
                Some((_, method)) => match method.code_mut() {
                    Some(code) => {
                        if index >= code.instructions.len() {
                            return Err(LuaError::runtime(format!("instruction index {index} out of bounds")));
                        }
                        code.replace_instruction(index, insn);
                        Ok(true)
                    }
                    None => Ok(false),
                },
                None => Ok(false),
            }
        })
    })?)?;

    index_table.set("insert_instruction", lua.create_function(|_, (tbl, index, insn_tbl): (LuaTable, usize, LuaTable)| {
        let insn = lua_insn::lua_to_instruction(&insn_tbl)?;
        with_method!(tbl, |w, mref| {
            match w.w().find_method_mut(&mref.class_descriptor, &mref.method_name) {
                Some((_, method)) => match method.code_mut() {
                    Some(code) => {
                        if index > code.instructions.len() {
                            return Err(LuaError::runtime(format!("instruction index {index} out of bounds")));
                        }
                        code.insert_instruction(index, insn);
                        Ok(true)
                    }
                    None => Ok(false),
                },
                None => Ok(false),
            }
        })
    })?)?;

    index_table.set("insert_instructions", lua.create_function(|_, (tbl, index, insns_tbl): (LuaTable, usize, LuaTable)| {
        let mut insns = Vec::new();
        for val in insns_tbl.sequence_values::<LuaTable>() {
            insns.push(lua_insn::lua_to_instruction(&val?)?);
        }
        with_method!(tbl, |w, mref| {
            match w.w().find_method_mut(&mref.class_descriptor, &mref.method_name) {
                Some((_, method)) => match method.code_mut() {
                    Some(code) => {
                        if index > code.instructions.len() {
                            return Err(LuaError::runtime(format!("instruction index {index} out of bounds")));
                        }
                        code.insert_instructions(index, &insns);
                        Ok(true)
                    }
                    None => Ok(false),
                },
                None => Ok(false),
            }
        })
    })?)?;

    index_table.set("remove_instruction", lua.create_function(|_, (tbl, index): (LuaTable, usize)| {
        with_method!(tbl, |w, mref| {
            match w.w().find_method_mut(&mref.class_descriptor, &mref.method_name) {
                Some((_, method)) => match method.code_mut() {
                    Some(code) => {
                        if index >= code.instructions.len() {
                            return Err(LuaError::runtime(format!("instruction index {index} out of bounds")));
                        }
                        code.remove_instruction(index);
                        Ok(true)
                    }
                    None => Ok(false),
                },
                None => Ok(false),
            }
        })
    })?)?;

    index_table.set("set_registers", lua.create_function(|_, (tbl, registers_size, outs_size): (LuaTable, u16, u16)| {
        with_method!(tbl, |w, mref| {
            match w.w().find_method_mut(&mref.class_descriptor, &mref.method_name) {
                Some((_, method)) => match method.code_mut() {
                    Some(code) => {
                        code.registers_size = registers_size;
                        code.outs_size = outs_size;
                        Ok(true)
                    }
                    None => Ok(false),
                },
                None => Ok(false),
            }
        })
    })?)?;

    meta.set("__index", index_table)?;
    t.set_metatable(Some(meta));

    Ok(t)
}

struct MethodMatchInfo {
    class_type: u32,
    method_idx: u32,
    access_flags: u32,
}

fn build_dex_table(lua: &Lua, ud: &LuaAnyUserData, dref: DexRef) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set("_ud", ud.clone())?;
    t.set("index", dref.index)?;
    t.set("_dref", lua.create_any_userdata(dref)?)?;

    let meta = lua.create_table()?;
    let index_table = lua.create_table()?;

    index_table.set("intern_string", lua.create_function(|_, (tbl, s): (LuaTable, String)| {
        with_dex!(tbl, |w, dref| {
            match w.w().dex_file_mut(dref.index) {
                Some(dex) => Ok(dex.intern_string(&s).0),
                None => Err(LuaError::runtime(format!("invalid dex index: {}", dref.index))),
            }
        })
    })?)?;

    index_table.set("intern_type", lua.create_function(|_, (tbl, desc): (LuaTable, String)| {
        with_dex!(tbl, |w, dref| {
            match w.w().dex_file_mut(dref.index) {
                Some(dex) => Ok(dex.intern_type(&desc).0),
                None => Err(LuaError::runtime(format!("invalid dex index: {}", dref.index))),
            }
        })
    })?)?;

    index_table.set("intern_proto", lua.create_function(|_, (tbl, desc): (LuaTable, String)| {
        with_dex!(tbl, |w, dref| {
            match w.w().dex_file_mut(dref.index) {
                Some(dex) => dex
                    .intern_proto(&desc)
                    .map(|p| p.0)
                    .map_err(|e| LuaError::runtime(e.to_string())),
                None => Err(LuaError::runtime(format!("invalid dex index: {}", dref.index))),
            }
        })
    })?)?;

    index_table.set("intern_method", lua.create_function(|_, (tbl, class, name, proto): (LuaTable, String, String, String)| {
        with_dex!(tbl, |w, dref| {
            match w.w().dex_file_mut(dref.index) {
                Some(dex) => dex
                    .intern_method(&class, &name, &proto)
                    .map(|m| m.0)
                    .map_err(|e| LuaError::runtime(e.to_string())),
                None => Err(LuaError::runtime(format!("invalid dex index: {}", dref.index))),
            }
        })
    })?)?;

    index_table.set("intern_field", lua.create_function(|_, (tbl, class, name, type_): (LuaTable, String, String, String)| {
        with_dex!(tbl, |w, dref| {
            match w.w().dex_file_mut(dref.index) {
                Some(dex) => dex
                    .intern_field(&class, &name, &type_)
                    .map(|f| f.0)
                    .map_err(|e| LuaError::runtime(e.to_string())),
                None => Err(LuaError::runtime(format!("invalid dex index: {}", dref.index))),
            }
        })
    })?)?;

    index_table.set("find_string_idx", lua.create_function(|_, (tbl, s): (LuaTable, String)| {
        with_dex!(tbl, |w, dref| {
            match w.r().dex_file(dref.index) {
                Some(dex) => Ok(dex.find_string_idx(&s).map(|idx| idx.0)),
                None => Err(LuaError::runtime(format!("invalid dex index: {}", dref.index))),
            }
        })
    })?)?;

    index_table.set("string", lua.create_function(|_, (tbl, str_idx): (LuaTable, u32)| {
        with_dex!(tbl, |w, dref| {
            match w.r().dex_file(dref.index) {
                Some(dex) => Ok(dex
                    .string(stitch_apk::stitch_dex::StringIdx(str_idx))
                    .to_owned()),
                None => Err(LuaError::runtime(format!("invalid dex index: {}", dref.index))),
            }
        })
    })?)?;

    index_table.set("type_descriptor", lua.create_function(|_, (tbl, type_idx): (LuaTable, u32)| {
        with_dex!(tbl, |w, dref| {
            match w.r().dex_file(dref.index) {
                Some(dex) => Ok(dex
                    .type_descriptor(stitch_apk::stitch_dex::TypeIdx(type_idx))
                    .to_owned()),
                None => Err(LuaError::runtime(format!("invalid dex index: {}", dref.index))),
            }
        })
    })?)?;

    index_table.set("build_lookups", lua.create_function(|_, tbl: LuaTable| {
        with_dex!(tbl, |w, dref| {
            match w.w().dex_file_mut(dref.index) {
                Some(dex) => {
                    dex.build_lookups();
                    Ok(())
                }
                None => Err(LuaError::runtime(format!("invalid dex index: {}", dref.index))),
            }
        })
    })?)?;

    meta.set("__index", index_table)?;
    t.set_metatable(Some(meta));

    Ok(t)
}

fn build_class_table(lua: &Lua, ud: &LuaAnyUserData, cref: ClassRef) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set("_ud", ud.clone())?;
    t.set("dex_index", cref.dex_index)?;
    t.set("descriptor", cref.descriptor.as_str())?;

    let dex_table = build_dex_table(lua, ud, DexRef { index: cref.dex_index })?;
    t.set("dex", dex_table)?;

    let descriptor = cref.descriptor.clone();
    t.set("_cref", lua.create_any_userdata(cref)?)?;

    let meta = lua.create_table()?;
    let index_table = lua.create_table()?;

    index_table.set("remove", lua.create_function(move |_, tbl: LuaTable| {
        let ud: LuaAnyUserData = tbl.get("_ud")?;
        let w = ud.borrow::<CtxPtr>()?;
        let cref: &ClassRef = &*tbl.get::<LuaAnyUserData>("_cref")?.borrow::<ClassRef>()?;
        match w.w().dex_file_mut(cref.dex_index) {
            Some(dex) => {
                let type_idx = match dex.find_type_idx(&descriptor) {
                    Some(idx) => idx,
                    None => return Ok(false),
                };
                Ok(dex.remove_class(type_idx).is_some())
            }
            None => Err(LuaError::runtime(format!("invalid dex index: {}", cref.dex_index))),
        }
    })?)?;

    meta.set("__index", index_table)?;
    t.set_metatable(Some(meta));

    Ok(t)
}

fn build_ctx_table(lua: &Lua, ud: LuaAnyUserData) -> Result<LuaTable> {
    let t = lua.create_table()?;
    t.set("_ud", ud)?;

    let m = lua.create_table()?;

    // ─── Lookups that return handles ────────────────────────────────────

    m.set("method", lua.create_function(|lua, (tbl, class_desc, method_name): (LuaTable, String, String)| {
        let ud: LuaAnyUserData = tbl.get("_ud")?;
        let w = ud.borrow::<CtxPtr>()?;

        let (dex_idx, method) = w.r().find_method(&class_desc, &method_name)
            .ok_or_else(|| LuaError::runtime(format!("{class_desc}.{method_name} not found")))?;

        let info = method.code.as_ref().map(|_| MethodMatchInfo {
            class_type: 0,
            method_idx: method.method.0,
            access_flags: method.access_flags.bits(),
        });

        drop(w);
        let ud: LuaAnyUserData = tbl.get("_ud")?;
        build_method_table(lua, &ud, MethodRef {
            class_descriptor: class_desc,
            method_name,
            dex_index: dex_idx,
        }, info)
    })?)?;

    m.set("class", lua.create_function(|lua, (tbl, descriptor): (LuaTable, String)| {
        let ud: LuaAnyUserData = tbl.get("_ud")?;
        let w = ud.borrow::<CtxPtr>()?;

        let (dex_idx, _) = w.r().find_class(&descriptor)
            .ok_or_else(|| LuaError::runtime(format!("class {descriptor} not found")))?;

        drop(w);
        let ud: LuaAnyUserData = tbl.get("_ud")?;
        build_class_table(lua, &ud, ClassRef {
            descriptor,
            dex_index: dex_idx,
        })
    })?)?;

    m.set("dex", lua.create_function(|lua, (tbl, index): (LuaTable, usize)| {
        let ud: LuaAnyUserData = tbl.get("_ud")?;
        let w = ud.borrow::<CtxPtr>()?;
        if w.r().dex_file(index).is_none() {
            return Err(LuaError::runtime(format!("invalid dex index: {index}")));
        }
        drop(w);
        let ud: LuaAnyUserData = tbl.get("_ud")?;
        build_dex_table(lua, &ud, DexRef { index })
    })?)?;

    // ─── Search methods that return handles ─────────────────────────────

    m.set("find_method_by_name", lua.create_function(|lua, (tbl, method_name): (LuaTable, String)| {
        let ud: LuaAnyUserData = tbl.get("_ud")?;
        let w = ud.borrow::<CtxPtr>()?;

        match w.r().find_method_by_name(&method_name) {
            Some((dex_idx, mm)) => {
                let dex = w.r().dex_file(dex_idx).ok_or_else(|| LuaError::runtime("invalid dex"))?;
                let class_desc = dex.type_descriptor(mm.class_idx).to_owned();
                let mname = dex.string(dex.methods[mm.method_idx.0 as usize].name).to_owned();
                let info = MethodMatchInfo {
                    class_type: mm.class_idx.0,
                    method_idx: mm.method_idx.0,
                    access_flags: mm.method.access_flags.bits(),
                };
                drop(w);
                let ud: LuaAnyUserData = tbl.get("_ud")?;
                Ok(Some(build_method_table(lua, &ud, MethodRef {
                    class_descriptor: class_desc,
                    method_name: mname,
                    dex_index: dex_idx,
                }, Some(info))?))
            }
            None => Ok(None),
        }
    })?)?;

    m.set("find_method", lua.create_function(|lua, (tbl, class_desc, method_name): (LuaTable, String, String)| {
        let ud: LuaAnyUserData = tbl.get("_ud")?;
        let w = ud.borrow::<CtxPtr>()?;

        match w.r().find_method(&class_desc, &method_name) {
            Some((dex_idx, method)) => {
                let info = MethodMatchInfo {
                    class_type: 0,
                    method_idx: method.method.0,
                    access_flags: method.access_flags.bits(),
                };
                drop(w);
                let ud: LuaAnyUserData = tbl.get("_ud")?;
                Ok(Some(build_method_table(lua, &ud, MethodRef {
                    class_descriptor: class_desc,
                    method_name,
                    dex_index: dex_idx,
                }, Some(info))?))
            }
            None => Ok(None),
        }
    })?)?;

    m.set("find_class", lua.create_function(|lua, (tbl, descriptor): (LuaTable, String)| {
        let ud: LuaAnyUserData = tbl.get("_ud")?;
        let w = ud.borrow::<CtxPtr>()?;

        match w.r().find_class(&descriptor) {
            Some((dex_idx, _)) => {
                drop(w);
                let ud: LuaAnyUserData = tbl.get("_ud")?;
                Ok(Some(build_class_table(lua, &ud, ClassRef {
                    descriptor,
                    dex_index: dex_idx,
                })?))
            }
            None => Ok(None),
        }
    })?)?;

    m.set("find_methods_with_opcodes", lua.create_function(|lua, (tbl, patterns): (LuaTable, LuaTable)| {
        let mut pats = Vec::new();
        for val in patterns.sequence_values::<String>() {
            pats.push(lua_insn::parse_pattern(&val?)?);
        }
        let ud: LuaAnyUserData = tbl.get("_ud")?;
        let w = ud.borrow::<CtxPtr>()?;
        let results = w.r().find_methods_with_opcodes(&pats);

        let mut handles = Vec::new();
        for (dex_idx, mm) in &results {
            let dex = w.r().dex_file(*dex_idx).ok_or_else(|| LuaError::runtime("invalid dex"))?;
            let class_desc = dex.type_descriptor(mm.class_idx).to_owned();
            let mname = dex.string(dex.methods[mm.method_idx.0 as usize].name).to_owned();
            handles.push((*dex_idx, class_desc, mname, MethodMatchInfo {
                class_type: mm.class_idx.0,
                method_idx: mm.method_idx.0,
                access_flags: mm.method.access_flags.bits(),
            }));
        }
        drop(w);

        let out = lua.create_table()?;
        let ud: LuaAnyUserData = tbl.get("_ud")?;
        for (i, (dex_idx, class_desc, mname, info)) in handles.into_iter().enumerate() {
            let handle = build_method_table(lua, &ud, MethodRef {
                class_descriptor: class_desc,
                method_name: mname,
                dex_index: dex_idx,
            }, Some(info))?;
            out.set(i + 1, handle)?;
        }
        Ok(out)
    })?)?;

    m.set("find_methods_by_strings", lua.create_function(|lua, (tbl, strings): (LuaTable, Vec<String>)| {
        let str_refs: Vec<&str> = strings.iter().map(|s| s.as_str()).collect();
        let ud: LuaAnyUserData = tbl.get("_ud")?;
        let w = ud.borrow::<CtxPtr>()?;
        let results = w.r().find_methods_by_strings(&str_refs);

        let mut handles = Vec::new();
        for (dex_idx, mm) in &results {
            let dex = w.r().dex_file(*dex_idx).ok_or_else(|| LuaError::runtime("invalid dex"))?;
            let class_desc = dex.type_descriptor(mm.class_idx).to_owned();
            let mname = dex.string(dex.methods[mm.method_idx.0 as usize].name).to_owned();
            handles.push((*dex_idx, class_desc, mname, MethodMatchInfo {
                class_type: mm.class_idx.0,
                method_idx: mm.method_idx.0,
                access_flags: mm.method.access_flags.bits(),
            }));
        }
        drop(w);

        let out = lua.create_table()?;
        let ud: LuaAnyUserData = tbl.get("_ud")?;
        for (i, (dex_idx, class_desc, mname, info)) in handles.into_iter().enumerate() {
            let handle = build_method_table(lua, &ud, MethodRef {
                class_descriptor: class_desc,
                method_name: mname,
                dex_index: dex_idx,
            }, Some(info))?;
            out.set(i + 1, handle)?;
        }
        Ok(out)
    })?)?;

    // ─── Manifest operations ────────────────────────────────────────────

    m.set("package_name", lua.create_function(|_, tbl: LuaTable| {
        with_ctx!(tbl, |w| Ok(w.r().package_name().map(|s| s.to_owned())))
    })?)?;

    m.set("version_code", lua.create_function(|_, tbl: LuaTable| {
        with_ctx!(tbl, |w| Ok(w.r().version_code()))
    })?)?;

    m.set("version_name", lua.create_function(|_, tbl: LuaTable| {
        with_ctx!(tbl, |w| Ok(w.r().version_name().map(|s| s.to_owned())))
    })?)?;

    m.set("min_sdk_version", lua.create_function(|_, tbl: LuaTable| {
        with_ctx!(tbl, |w| Ok(w.r().manifest().min_sdk_version()))
    })?)?;

    m.set("split_name", lua.create_function(|_, tbl: LuaTable| {
        with_ctx!(tbl, |w| Ok(w.r().manifest().split_name().map(|s| s.to_owned())))
    })?)?;

    m.set("dex_count", lua.create_function(|_, tbl: LuaTable| {
        with_ctx!(tbl, |w| Ok(w.r().dex_count()))
    })?)?;

    m.set("set_version_code", lua.create_function(|_, (tbl, code): (LuaTable, u32)| {
        with_ctx!(tbl, |w| { w.w().manifest_mut().set_version_code(code); Ok(()) })
    })?)?;

    m.set("set_version_name", lua.create_function(|_, (tbl, name): (LuaTable, String)| {
        with_ctx!(tbl, |w| { w.w().manifest_mut().set_version_name(&name); Ok(()) })
    })?)?;

    m.set("set_min_sdk", lua.create_function(|_, (tbl, sdk): (LuaTable, u32)| {
        with_ctx!(tbl, |w| { w.w().manifest_mut().set_min_sdk(sdk); Ok(()) })
    })?)?;

    m.set("add_permission", lua.create_function(|_, (tbl, perm): (LuaTable, String)| {
        with_ctx!(tbl, |w| { w.w().manifest_mut().add_permission(&perm); Ok(()) })
    })?)?;

    m.set("set_attribute_int", lua.create_function(|_, (tbl, element, res_id, value): (LuaTable, String, u32, i32)| {
        with_ctx!(tbl, |w| { w.w().manifest_mut().set_attribute_int(&element, res_id, value); Ok(()) })
    })?)?;

    m.set("set_attribute_string", lua.create_function(|_, (tbl, element, res_id, value): (LuaTable, String, u32, String)| {
        with_ctx!(tbl, |w| { w.w().manifest_mut().set_attribute_string(&element, res_id, &value); Ok(()) })
    })?)?;

    // ─── Resource operations ────────────────────────────────────────────

    m.set("has_resources", lua.create_function(|_, tbl: LuaTable| {
        with_ctx!(tbl, |w| Ok(w.r().resources().is_some()))
    })?)?;

    m.set("resource_string", lua.create_function(|_, (tbl, index): (LuaTable, u32)| {
        with_ctx!(tbl, |w| {
            match w.r().resources() {
                Some(res) => Ok(res.get_string(index).map(|s| s.to_owned())),
                None => Err(LuaError::runtime("no resources.arsc")),
            }
        })
    })?)?;

    m.set("set_resource_string", lua.create_function(|_, (tbl, index, value): (LuaTable, u32, String)| {
        with_ctx!(tbl, |w| {
            match w.w().resources_mut() {
                Some(res) => { res.set_string(index, value); Ok(()) }
                None => Err(LuaError::runtime("no resources.arsc")),
            }
        })
    })?)?;

    m.set("find_resource_entries_by_string", lua.create_function(|lua, (tbl, string_index): (LuaTable, u32)| {
        with_ctx!(tbl, |w| {
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
        })
    })?)?;

    m.set("replace_resource_entry_string", lua.create_function(|_, (tbl, res_id, new_string_index): (LuaTable, u32, u32)| {
        with_ctx!(tbl, |w| {
            match w.w().resources_mut() {
                Some(res) => { res.replace_entry_string(res_id, new_string_index); Ok(()) }
                None => Err(LuaError::runtime("no resources.arsc")),
            }
        })
    })?)?;

    m.set("merge_extension_dex", lua.create_function(|_, (tbl, paths): (LuaTable, Vec<String>)| {
        with_ctx!(tbl, |w| {
            w.w().merge_extension_dex(&paths).map_err(|e| LuaError::runtime(e.to_string()))
        })
    })?)?;

    let meta = lua.create_table()?;
    meta.set("__index", m)?;
    t.set_metatable(Some(meta));

    Ok(t)
}
