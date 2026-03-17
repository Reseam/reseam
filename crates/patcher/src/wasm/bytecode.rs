use stitch_apk::stitch_dex::{
    AccessFlags, CodeItem, DexFile, EncodedField, EncodedMethod, FieldIdx,
    Fingerprint, Instruction as DexInsn, InstructionPattern, StringIdx,
};

use super::convert::{dex_to_wit, wit_to_dex, WitFieldRef, WitInstruction, WitMethodRef};
use super::stitch::patch::bytecode::Host;
use super::stitch::patch::types::{
    self, ClassInfo, FieldInfo, FingerprintMatch, InstructionHit, MethodInfo, NewField, NewMethod,
};
use super::{WasmState, find_method_location, get_method_mut, get_method_ref, method_match_location};

fn wit_to_access_flags(flags: types::AccessFlags) -> AccessFlags {
    let mut af = AccessFlags::empty();
    if flags.contains(types::AccessFlags::PUBLIC) { af |= AccessFlags::PUBLIC; }
    if flags.contains(types::AccessFlags::PRIVATE) { af |= AccessFlags::PRIVATE; }
    if flags.contains(types::AccessFlags::PROTECTED) { af |= AccessFlags::PROTECTED; }
    if flags.contains(types::AccessFlags::STATIC) { af |= AccessFlags::STATIC; }
    if flags.contains(types::AccessFlags::FINAL) { af |= AccessFlags::FINAL; }
    if flags.contains(types::AccessFlags::SYNCHRONIZED) { af |= AccessFlags::SYNCHRONIZED; }
    if flags.contains(types::AccessFlags::VOLATILE_OR_BRIDGE) { af |= AccessFlags::VOLATILE; }
    if flags.contains(types::AccessFlags::TRANSIENT_OR_VARARGS) { af |= AccessFlags::TRANSIENT; }
    if flags.contains(types::AccessFlags::NATIVE) { af |= AccessFlags::NATIVE; }
    if flags.contains(types::AccessFlags::INTERFACE) { af |= AccessFlags::INTERFACE; }
    if flags.contains(types::AccessFlags::ABSTRACT) { af |= AccessFlags::ABSTRACT; }
    if flags.contains(types::AccessFlags::STRICT) { af |= AccessFlags::STRICT; }
    if flags.contains(types::AccessFlags::SYNTHETIC) { af |= AccessFlags::SYNTHETIC; }
    if flags.contains(types::AccessFlags::ANNOTATION) { af |= AccessFlags::ANNOTATION; }
    if flags.contains(types::AccessFlags::ENUM) { af |= AccessFlags::ENUM; }
    if flags.contains(types::AccessFlags::CONSTRUCTOR) { af |= AccessFlags::CONSTRUCTOR; }
    if flags.contains(types::AccessFlags::DECLARED_SYNCHRONIZED) { af |= AccessFlags::DECLARED_SYNCHRONIZED; }
    af
}

fn access_flags_to_wit(flags: AccessFlags) -> types::AccessFlags {
    let mut wf = types::AccessFlags::empty();
    if flags.contains(AccessFlags::PUBLIC) { wf |= types::AccessFlags::PUBLIC; }
    if flags.contains(AccessFlags::PRIVATE) { wf |= types::AccessFlags::PRIVATE; }
    if flags.contains(AccessFlags::PROTECTED) { wf |= types::AccessFlags::PROTECTED; }
    if flags.contains(AccessFlags::STATIC) { wf |= types::AccessFlags::STATIC; }
    if flags.contains(AccessFlags::FINAL) { wf |= types::AccessFlags::FINAL; }
    if flags.contains(AccessFlags::SYNCHRONIZED) { wf |= types::AccessFlags::SYNCHRONIZED; }
    if flags.contains(AccessFlags::VOLATILE) { wf |= types::AccessFlags::VOLATILE_OR_BRIDGE; }
    if flags.contains(AccessFlags::BRIDGE) { wf |= types::AccessFlags::VOLATILE_OR_BRIDGE; }
    if flags.contains(AccessFlags::TRANSIENT) { wf |= types::AccessFlags::TRANSIENT_OR_VARARGS; }
    if flags.contains(AccessFlags::VARARGS) { wf |= types::AccessFlags::TRANSIENT_OR_VARARGS; }
    if flags.contains(AccessFlags::NATIVE) { wf |= types::AccessFlags::NATIVE; }
    if flags.contains(AccessFlags::INTERFACE) { wf |= types::AccessFlags::INTERFACE; }
    if flags.contains(AccessFlags::ABSTRACT) { wf |= types::AccessFlags::ABSTRACT; }
    if flags.contains(AccessFlags::STRICT) { wf |= types::AccessFlags::STRICT; }
    if flags.contains(AccessFlags::SYNTHETIC) { wf |= types::AccessFlags::SYNTHETIC; }
    if flags.contains(AccessFlags::ANNOTATION) { wf |= types::AccessFlags::ANNOTATION; }
    if flags.contains(AccessFlags::ENUM) { wf |= types::AccessFlags::ENUM; }
    if flags.contains(AccessFlags::CONSTRUCTOR) { wf |= types::AccessFlags::CONSTRUCTOR; }
    if flags.contains(AccessFlags::DECLARED_SYNCHRONIZED) { wf |= types::AccessFlags::DECLARED_SYNCHRONIZED; }
    wf
}

fn scan_location(ctx: &crate::context::PatchContext<'_>, dex_idx: usize, class_idx: usize, method_idx: usize) -> (usize, bool) {
    let dex = ctx.dex_file(dex_idx).expect("dex");
    let class = &dex.classes[class_idx];
    let data = class.class_data.as_ref().expect("class data");
    let is_virtual = method_idx >= data.direct_methods.len();
    let actual = if is_virtual { method_idx - data.direct_methods.len() } else { method_idx };
    (actual, is_virtual)
}

impl Host for WasmState {
    // ── Method lookup ──

    fn find_method(&mut self, class_descriptor: String, method_name: String) -> Option<u32> {
        let ctx = self.ctx();
        let result = ctx.find_method(&class_descriptor, &method_name);
        match result {
            Some((dex_idx, method)) => {
                let (ci, mi, iv) = find_method_location(ctx, dex_idx, method);
                Some(self.handles.alloc_method(dex_idx, ci, mi, iv))
            }
            None => None,
        }
    }

    fn find_method_by_name(&mut self, name: String) -> Option<u32> {
        let ctx = self.ctx();
        let result = ctx.find_method_by_name(&name);
        match result {
            Some((dex_idx, mm)) => {
                let (ci, mi, iv) = method_match_location(ctx, dex_idx, &mm);
                Some(self.handles.alloc_method(dex_idx, ci, mi, iv))
            }
            None => None,
        }
    }

    fn find_methods_by_strings(&mut self, strings: Vec<String>) -> Vec<u32> {
        let locations: Vec<(usize, usize, usize, bool)> = {
            let ctx = self.ctx();
            let str_refs: Vec<&str> = strings.iter().map(|s| s.as_str()).collect();
            let matches = ctx.find_methods_by_strings(&str_refs);
            matches.iter().map(|(dex_idx, mm)| {
                let (ci, mi, iv) = method_match_location(ctx, *dex_idx, mm);
                (*dex_idx, ci, mi, iv)
            }).collect()
        };
        locations.into_iter().map(|(di, ci, mi, iv)| {
            self.handles.alloc_method(di, ci, mi, iv)
        }).collect()
    }

    fn find_methods_by_opcodes(&mut self, pattern: Vec<Option<u16>>) -> Vec<u32> {
        let ip: Vec<InstructionPattern> = pattern.iter().map(|o| match o {
            None => InstructionPattern::Any,
            Some(op) => InstructionPattern::OpcodeValue(*op),
        }).collect();
        let locations: Vec<(usize, usize, usize, bool)> = {
            let ctx = self.ctx();
            let matches = ctx.find_methods_with_opcodes(&ip);
            matches.iter().map(|(dex_idx, mm)| {
                let (ci, mi, iv) = method_match_location(ctx, *dex_idx, mm);
                (*dex_idx, ci, mi, iv)
            }).collect()
        };
        locations.into_iter().map(|(di, ci, mi, iv)| {
            self.handles.alloc_method(di, ci, mi, iv)
        }).collect()
    }

    fn find_method_by_fingerprint(&mut self, fp: types::Fingerprint) -> Option<FingerprintMatch> {
        let dex_fp = convert_fingerprint(&fp);
        let (dex_idx, ci, mi, iv, matched_indices) = {
            let ctx = self.ctx();
            let (dex_idx, fm) = ctx.find_method_by_fingerprint(&dex_fp)?;
            let (ci, mi, iv) = find_method_location(ctx, dex_idx, fm.method);
            (dex_idx, ci, mi, iv, fm.matched_indices.clone())
        };
        let mh = self.handles.alloc_method(dex_idx, ci, mi, iv);
        Some(FingerprintMatch {
            method: mh,
            matched_indices,
        })
    }

    fn find_methods_by_fingerprint(&mut self, fp: types::Fingerprint) -> Vec<FingerprintMatch> {
        let dex_fp = convert_fingerprint(&fp);
        let ctx = self.ctx();
        let matches = ctx.find_methods_by_fingerprint(&dex_fp);
        let locs: Vec<_> = matches.iter().map(|(dex_idx, fm)| {
            let (ci, mi, iv) = find_method_location(ctx, *dex_idx, fm.method);
            (*dex_idx, ci, mi, iv, fm.matched_indices.clone())
        }).collect();
        locs.into_iter().map(|(di, ci, mi, iv, indices)| {
            let mh = self.handles.alloc_method(di, ci, mi, iv);
            FingerprintMatch { method: mh, matched_indices: indices }
        }).collect()
    }

    // ── Class lookup ──

    fn find_class(&mut self, descriptor: String) -> Option<u32> {
        let ctx = self.ctx();
        match ctx.find_class(&descriptor) {
            Some((dex_idx, class)) => {
                let dex = ctx.dex_file(dex_idx).expect("dex");
                let class_idx = dex.classes.iter().position(|c| std::ptr::eq(c, class)).expect("class");
                Some(self.handles.alloc_class(dex_idx, class_idx))
            }
            None => None,
        }
    }

    // ── Info queries ──

    fn get_method_info(&mut self, m: u32) -> MethodInfo {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file(mh.dex_idx).expect("dex");
        let class = &dex.classes[mh.class_idx];
        let method = get_method_ref(dex, mh);
        let method_id = &dex.methods[method.method.0 as usize];
        let class_desc = dex.type_descriptor(class.class_type).to_string();
        let name = dex.string(method_id.name).to_string();
        let proto_def = &dex.prototypes[method_id.proto.0 as usize];
        let ret = dex.type_descriptor(proto_def.return_type);
        let params: Vec<&str> = proto_def.parameters.iter().map(|p| dex.type_descriptor(*p)).collect();
        let proto = format!("({}){}", params.join(""), ret);
        let (reg_count, ins, outs, insn_count) = match &method.code {
            Some(c) => (c.registers_size, c.ins_size, c.outs_size, c.instructions.len() as u32),
            None => (0, 0, 0, 0),
        };
        MethodInfo {
            class_descriptor: class_desc,
            method_name: name,
            proto,
            access_flags: access_flags_to_wit(method.access_flags),
            dex_index: mh.dex_idx as u32,
            register_count: reg_count,
            ins_size: ins,
            outs_size: outs,
            instruction_count: insn_count,
        }
    }

    fn get_class_info(&mut self, c: u32) -> ClassInfo {
        let ch = self.handles.get_class(c).expect("valid class handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file(ch.dex_idx).expect("dex");
        let class = &dex.classes[ch.class_idx];
        let desc = dex.type_descriptor(class.class_type).to_string();
        let superclass = class.superclass.map(|s| dex.type_descriptor(s).to_string());
        let interfaces: Vec<String> = class.interfaces.iter().map(|i| dex.type_descriptor(*i).to_string()).collect();
        let (dm, vm, sf, inf) = match &class.class_data {
            Some(d) => (d.direct_methods.len() as u32, d.virtual_methods.len() as u32, d.static_fields.len() as u32, d.instance_fields.len() as u32),
            None => (0, 0, 0, 0),
        };
        ClassInfo {
            descriptor: desc,
            access_flags: access_flags_to_wit(class.access_flags),
            superclass,
            interfaces,
            dex_index: ch.dex_idx as u32,
            direct_method_count: dm,
            virtual_method_count: vm,
            static_field_count: sf,
            instance_field_count: inf,
        }
    }

    fn class_methods(&mut self, c: u32) -> Vec<u32> {
        let ch = self.handles.get_class(c).expect("valid class handle");
        let (dm_count, vm_count) = {
            let ctx = self.ctx();
            let dex = ctx.dex_file(ch.dex_idx).expect("dex");
            let class = &dex.classes[ch.class_idx];
            match &class.class_data {
                Some(d) => (d.direct_methods.len(), d.virtual_methods.len()),
                None => (0, 0),
            }
        };
        let mut handles = Vec::with_capacity(dm_count + vm_count);
        for i in 0..dm_count {
            handles.push(self.handles.alloc_method(ch.dex_idx, ch.class_idx, i, false));
        }
        for i in 0..vm_count {
            handles.push(self.handles.alloc_method(ch.dex_idx, ch.class_idx, i, true));
        }
        handles
    }

    fn class_direct_methods(&mut self, c: u32) -> Vec<u32> {
        let ch = self.handles.get_class(c).expect("valid class handle");
        let dm_count = {
            let ctx = self.ctx();
            let dex = ctx.dex_file(ch.dex_idx).expect("dex");
            let class = &dex.classes[ch.class_idx];
            class.class_data.as_ref().map(|d| d.direct_methods.len()).unwrap_or(0)
        };
        (0..dm_count).map(|i| self.handles.alloc_method(ch.dex_idx, ch.class_idx, i, false)).collect()
    }

    fn class_virtual_methods(&mut self, c: u32) -> Vec<u32> {
        let ch = self.handles.get_class(c).expect("valid class handle");
        let vm_count = {
            let ctx = self.ctx();
            let dex = ctx.dex_file(ch.dex_idx).expect("dex");
            let class = &dex.classes[ch.class_idx];
            class.class_data.as_ref().map(|d| d.virtual_methods.len()).unwrap_or(0)
        };
        (0..vm_count).map(|i| self.handles.alloc_method(ch.dex_idx, ch.class_idx, i, true)).collect()
    }

    fn class_fields(&mut self, c: u32) -> Vec<FieldInfo> {
        let ch = self.handles.get_class(c).expect("valid class handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file(ch.dex_idx).expect("dex");
        let class = &dex.classes[ch.class_idx];
        collect_fields(dex, class, true, true)
    }

    fn class_static_fields(&mut self, c: u32) -> Vec<FieldInfo> {
        let ch = self.handles.get_class(c).expect("valid class handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file(ch.dex_idx).expect("dex");
        let class = &dex.classes[ch.class_idx];
        collect_fields(dex, class, true, false)
    }

    fn class_instance_fields(&mut self, c: u32) -> Vec<FieldInfo> {
        let ch = self.handles.get_class(c).expect("valid class handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file(ch.dex_idx).expect("dex");
        let class = &dex.classes[ch.class_idx];
        collect_fields(dex, class, false, true)
    }

    // ── Instruction read ──

    fn get_instructions(&mut self, m: u32) -> Vec<WitInstruction> {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file(mh.dex_idx).expect("dex");
        let method = get_method_ref(dex, mh);
        match &method.code {
            Some(c) => c.instructions.iter().map(|insn| dex_to_wit(insn, dex)).collect(),
            None => Vec::new(),
        }
    }

    fn get_instruction(&mut self, m: u32, index: u32) -> WitInstruction {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file(mh.dex_idx).expect("dex");
        let method = get_method_ref(dex, mh);
        match &method.code {
            Some(c) => match c.instructions.get(index as usize) {
                Some(insn) => dex_to_wit(insn, dex),
                None => WitInstruction::Simple(0x00),
            },
            None => WitInstruction::Simple(0x00),
        }
    }

    fn instruction_count(&mut self, m: u32) -> u32 {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file(mh.dex_idx).expect("dex");
        let method = get_method_ref(dex, mh);
        method.code.as_ref().map(|c| c.instructions.len() as u32).unwrap_or(0)
    }

    // ── Instruction search ──

    fn index_of_first(&mut self, m: u32, start: u32, op: u16) -> Option<u32> {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file(mh.dex_idx).expect("dex");
        let method = get_method_ref(dex, mh);
        method.code.as_ref().and_then(|c| {
            c.instructions.iter().enumerate().skip(start as usize)
                .find(|(_, insn)| insn.opcode() == Some(op))
                .map(|(i, _)| i as u32)
        })
    }

    fn index_of_first_reversed(&mut self, m: u32, start: u32, op: u16) -> Option<u32> {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file(mh.dex_idx).expect("dex");
        let method = get_method_ref(dex, mh);
        method.code.as_ref().and_then(|c| {
            let end = (start as usize).min(c.instructions.len());
            c.instructions[..end].iter().enumerate().rev()
                .find(|(_, insn)| insn.opcode() == Some(op))
                .map(|(i, _)| i as u32)
        })
    }

    fn index_of_first_literal(&mut self, m: u32, literal: i64) -> Option<u32> {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file(mh.dex_idx).expect("dex");
        let method = get_method_ref(dex, mh);
        method.code.as_ref().and_then(|c| {
            c.instructions.iter().position(|insn| insn.literal() == Some(literal)).map(|i| i as u32)
        })
    }

    fn index_of_first_string(&mut self, m: u32, s: String) -> Option<u32> {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file(mh.dex_idx).expect("dex");
        let target_idx = dex.find_string_idx(&s)?;
        let method = get_method_ref(dex, mh);
        method.code.as_ref().and_then(|c| {
            c.instructions.iter().position(|insn| insn.string_ref() == Some(target_idx)).map(|i| i as u32)
        })
    }

    fn find_all_indices(&mut self, m: u32, op: u16) -> Vec<u32> {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file(mh.dex_idx).expect("dex");
        let method = get_method_ref(dex, mh);
        match &method.code {
            Some(c) => c.instructions.iter().enumerate()
                .filter(|(_, insn)| insn.opcode() == Some(op))
                .map(|(i, _)| i as u32)
                .collect(),
            None => Vec::new(),
        }
    }

    // ── Global instruction scanning ──

    fn find_instructions_by_literal(&mut self, literal: i64) -> Vec<InstructionHit> {
        let locations: Vec<(usize, usize, usize, bool, usize)> = {
            let ctx = self.ctx();
            let hits = ctx.find_instructions_by_literal(literal);
            hits.iter().map(|(dex_idx, class_idx, method_idx, insn_idx)| {
                let (actual, is_virtual) = scan_location(ctx, *dex_idx, *class_idx, *method_idx);
                (*dex_idx, *class_idx, actual, is_virtual, *insn_idx)
            }).collect()
        };
        locations.into_iter().map(|(di, ci, mi, iv, ii)| {
            InstructionHit {
                method: self.handles.alloc_method(di, ci, mi, iv),
                index: ii as u32,
            }
        }).collect()
    }

    fn find_instructions_by_string(&mut self, s: String) -> Vec<InstructionHit> {
        let locations: Vec<(usize, usize, usize, bool, usize)> = {
            let ctx = self.ctx();
            let hits = ctx.find_instructions_by_string(&s);
            hits.iter().map(|(dex_idx, class_idx, method_idx, insn_idx)| {
                let (actual, is_virtual) = scan_location(ctx, *dex_idx, *class_idx, *method_idx);
                (*dex_idx, *class_idx, actual, is_virtual, *insn_idx)
            }).collect()
        };
        locations.into_iter().map(|(di, ci, mi, iv, ii)| {
            InstructionHit {
                method: self.handles.alloc_method(di, ci, mi, iv),
                index: ii as u32,
            }
        }).collect()
    }

    fn find_instructions_by_resource_id(&mut self, res_type: String, res_name: String) -> Vec<InstructionHit> {
        let res_id = match self.ctx().find_resource_id(&res_type, &res_name) {
            Some(id) => id,
            None => return Vec::new(),
        };
        self.find_instructions_by_literal(res_id as i64)
    }

    // ── Instruction mutation ──

    fn set_instructions(&mut self, m: u32, insns: Vec<WitInstruction>) {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(mh.dex_idx).expect("dex");
        let dex_insns: Vec<DexInsn> = insns.iter().filter_map(|wi| wit_to_dex(wi, dex).ok()).collect();
        let method = get_method_mut(dex, mh);
        if let Some(code) = &mut method.code {
            code.set_instructions(dex_insns);
        }
    }

    fn insert_instruction(&mut self, m: u32, index: u32, insn: WitInstruction) {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(mh.dex_idx).expect("dex");
        if let Ok(dex_insn) = wit_to_dex(&insn, dex) {
            let method = get_method_mut(dex, mh);
            if let Some(code) = &mut method.code {
                code.insert_instruction(index as usize, dex_insn);
            }
        }
    }

    fn insert_instructions(&mut self, m: u32, index: u32, insns: Vec<WitInstruction>) {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(mh.dex_idx).expect("dex");
        let dex_insns: Vec<DexInsn> = insns.iter().filter_map(|wi| wit_to_dex(wi, dex).ok()).collect();
        let method = get_method_mut(dex, mh);
        if let Some(code) = &mut method.code {
            code.insert_instructions(index as usize, &dex_insns);
        }
    }

    fn replace_instruction(&mut self, m: u32, index: u32, insn: WitInstruction) {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(mh.dex_idx).expect("dex");
        if let Ok(dex_insn) = wit_to_dex(&insn, dex) {
            let method = get_method_mut(dex, mh);
            if let Some(code) = &mut method.code {
                code.replace_instruction(index as usize, dex_insn);
            }
        }
    }

    fn remove_instruction(&mut self, m: u32, index: u32) {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(mh.dex_idx).expect("dex");
        let method = get_method_mut(dex, mh);
        if let Some(code) = &mut method.code {
            code.remove_instruction(index as usize);
        }
    }

    fn remove_instructions(&mut self, m: u32, index: u32, count: u32) {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(mh.dex_idx).expect("dex");
        let method = get_method_mut(dex, mh);
        if let Some(code) = &mut method.code {
            for _ in 0..count {
                if (index as usize) < code.instructions.len() {
                    code.remove_instruction(index as usize);
                }
            }
        }
    }

    fn return_early(&mut self, m: u32) {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(mh.dex_idx).expect("dex");
        let method = get_method_mut(dex, mh);
        method.return_early();
    }

    fn return_early_int(&mut self, m: u32, value: i32) {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(mh.dex_idx).expect("dex");
        let method = get_method_mut(dex, mh);
        method.return_early_int(value);
    }

    // ── Register manipulation ──

    fn set_registers(&mut self, m: u32, registers_size: u16, outs_size: u16) {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(mh.dex_idx).expect("dex");
        let method = get_method_mut(dex, mh);
        if let Some(code) = &mut method.code {
            code.registers_size = registers_size;
            code.outs_size = outs_size;
        }
    }

    fn registers_size(&mut self, m: u32) -> u16 {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file(mh.dex_idx).expect("dex");
        get_method_ref(dex, mh).code.as_ref().map(|c| c.registers_size).unwrap_or(0)
    }

    fn ins_size(&mut self, m: u32) -> u16 {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file(mh.dex_idx).expect("dex");
        get_method_ref(dex, mh).code.as_ref().map(|c| c.ins_size).unwrap_or(0)
    }

    fn outs_size(&mut self, m: u32) -> u16 {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file(mh.dex_idx).expect("dex");
        get_method_ref(dex, mh).code.as_ref().map(|c| c.outs_size).unwrap_or(0)
    }

    fn find_free_register(&mut self, m: u32, at_index: u32, exclude: Vec<u16>) -> u16 {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file(mh.dex_idx).expect("dex");
        let method = get_method_ref(dex, mh);
        method.code.as_ref()
            .and_then(|c| ctx.find_free_register(c, at_index as usize, &exclude))
            .unwrap_or(0)
    }

    fn find_free_registers(&mut self, m: u32, at_index: u32, count: u32, exclude: Vec<u16>) -> Vec<u16> {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file(mh.dex_idx).expect("dex");
        let method = get_method_ref(dex, mh);
        method.code.as_ref()
            .and_then(|c| ctx.find_free_registers(c, at_index as usize, count as usize, &exclude))
            .unwrap_or_default()
    }

    // ── Instruction register read ──

    fn instruction_register_a(&mut self, m: u32, index: u32) -> u16 {
        get_insn_register(self, m, index, 0)
    }

    fn instruction_register_b(&mut self, m: u32, index: u32) -> u16 {
        get_insn_register(self, m, index, 1)
    }

    fn instruction_register_c(&mut self, m: u32, index: u32) -> u16 {
        get_insn_register(self, m, index, 2)
    }

    fn instruction_register_d(&mut self, m: u32, index: u32) -> u16 {
        get_insn_register(self, m, index, 3)
    }

    fn instruction_wide_literal(&mut self, m: u32, index: u32) -> i64 {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file(mh.dex_idx).expect("dex");
        let method = get_method_ref(dex, mh);
        method.code.as_ref()
            .and_then(|c| c.instructions.get(index as usize).and_then(|insn| insn.literal()))
            .unwrap_or(0)
    }

    fn instruction_string_ref(&mut self, m: u32, index: u32) -> Option<String> {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file(mh.dex_idx).expect("dex");
        let method = get_method_ref(dex, mh);
        method.code.as_ref().and_then(|c| {
            c.instructions.get(index as usize).and_then(|insn| {
                insn.string_ref().map(|idx| dex.string(idx).to_string())
            })
        })
    }

    fn instruction_method_ref(&mut self, m: u32, index: u32) -> Option<WitMethodRef> {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file(mh.dex_idx).expect("dex");
        let method = get_method_ref(dex, mh);
        method.code.as_ref().and_then(|c| {
            c.instructions.get(index as usize).and_then(|insn| {
                insn.method_ref().map(|method_idx| {
                    let mid = &dex.methods[method_idx.0 as usize];
                    let class = dex.type_descriptor(mid.class).to_string();
                    let name = dex.string(mid.name).to_string();
                    let proto = &dex.prototypes[mid.proto.0 as usize];
                    let ret = dex.type_descriptor(proto.return_type);
                    let params: Vec<&str> = proto.parameters.iter().map(|p| dex.type_descriptor(*p)).collect();
                    WitMethodRef {
                        defining_class: class,
                        name,
                        proto: format!("({}){}", params.join(""), ret),
                    }
                })
            })
        })
    }

    fn instruction_field_ref(&mut self, m: u32, index: u32) -> Option<WitFieldRef> {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file(mh.dex_idx).expect("dex");
        let method = get_method_ref(dex, mh);
        method.code.as_ref().and_then(|c| {
            c.instructions.get(index as usize).and_then(|insn| {
                insn.field_ref().map(|field_idx| {
                    let fid = &dex.fields[field_idx.0 as usize];
                    WitFieldRef {
                        defining_class: dex.type_descriptor(fid.class).to_string(),
                        name: dex.string(fid.name).to_string(),
                        field_type: dex.type_descriptor(fid.type_).to_string(),
                    }
                })
            })
        })
    }

    fn instruction_type_ref(&mut self, m: u32, index: u32) -> Option<String> {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file(mh.dex_idx).expect("dex");
        let method = get_method_ref(dex, mh);
        method.code.as_ref().and_then(|c| {
            c.instructions.get(index as usize).and_then(|insn| {
                insn.type_ref().map(|type_idx| dex.type_descriptor(type_idx).to_string())
            })
        })
    }

    // ── Class mutation ──

    fn set_class_access_flags(&mut self, c: u32, flags: types::AccessFlags) {
        let ch = self.handles.get_class(c).expect("valid class handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(ch.dex_idx).expect("dex");
        dex.classes[ch.class_idx].access_flags = wit_to_access_flags(flags);
    }

    fn set_superclass(&mut self, c: u32, superclass: String) {
        let ch = self.handles.get_class(c).expect("valid class handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(ch.dex_idx).expect("dex");
        let _ = dex.set_superclass(ch.class_idx, &superclass);
    }

    fn add_interface(&mut self, c: u32, interface_descriptor: String) {
        let ch = self.handles.get_class(c).expect("valid class handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(ch.dex_idx).expect("dex");
        let type_idx = dex.intern_type(&interface_descriptor);
        dex.classes[ch.class_idx].interfaces.push(type_idx);
    }

    fn remove_class(&mut self, c: u32) {
        let ch = self.handles.get_class(c).expect("valid class handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(ch.dex_idx).expect("dex");
        let class_type = dex.classes[ch.class_idx].class_type;
        dex.remove_class(class_type);
    }

    fn create_class(&mut self, dex_index: u32, descriptor: String, flags: types::AccessFlags, superclass: String) -> u32 {
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(dex_index as usize).expect("dex");
        let af = wit_to_access_flags(flags);
        match dex.create_class(&descriptor, af, Some(&superclass)) {
            Ok(class_idx) => self.handles.alloc_class(dex_index as usize, class_idx),
            Err(_) => 0,
        }
    }

    fn add_method(&mut self, c: u32, method: NewMethod) -> u32 {
        let ch = self.handles.get_class(c).expect("valid class handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(ch.dex_idx).expect("dex");
        let class_desc = dex.type_descriptor(dex.classes[ch.class_idx].class_type).to_string();
        let method_idx = match dex.intern_method(&class_desc, &method.name, &method.proto) {
            Ok(idx) => idx,
            Err(_) => return 0,
        };
        let af = wit_to_access_flags(method.access_flags);
        let insns: Vec<DexInsn> = method.instructions.iter()
            .filter_map(|wi| wit_to_dex(wi, dex).ok())
            .collect();
        let code = CodeItem {
            registers_size: method.registers_size,
            ins_size: method.ins_size,
            outs_size: method.outs_size,
            debug_info: None,
            instructions: insns,
            tries: Vec::new(),
            catch_handlers: Vec::new(),
        };
        let em = EncodedMethod {
            method: method_idx,
            access_flags: af,
            code: Some(code),
        };
        let is_virtual = !af.contains(AccessFlags::STATIC) && !af.contains(AccessFlags::CONSTRUCTOR) && !af.intersects(AccessFlags::PRIVATE);
        let class = &mut dex.classes[ch.class_idx];
        let mi = if is_virtual {
            class.add_virtual_method(em);
            class.class_data.as_ref().map(|d| d.virtual_methods.len() - 1).unwrap_or(0)
        } else {
            class.add_direct_method(em);
            class.class_data.as_ref().map(|d| d.direct_methods.len() - 1).unwrap_or(0)
        };
        self.handles.alloc_method(ch.dex_idx, ch.class_idx, mi, is_virtual)
    }

    fn remove_method(&mut self, m: u32) {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(mh.dex_idx).expect("dex");
        let class = &mut dex.classes[mh.class_idx];
        if let Some(data) = &mut class.class_data {
            if mh.is_virtual {
                if mh.method_idx < data.virtual_methods.len() {
                    data.virtual_methods.remove(mh.method_idx);
                }
            } else if mh.method_idx < data.direct_methods.len() {
                data.direct_methods.remove(mh.method_idx);
            }
        }
    }

    fn set_method_access_flags(&mut self, m: u32, flags: types::AccessFlags) {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(mh.dex_idx).expect("dex");
        let method = get_method_mut(dex, mh);
        method.access_flags = wit_to_access_flags(flags);
    }

    fn add_field(&mut self, c: u32, field: NewField) -> u32 {
        let ch = self.handles.get_class(c).expect("valid class handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(ch.dex_idx).expect("dex");
        let class_desc = dex.type_descriptor(dex.classes[ch.class_idx].class_type).to_string();
        let field_idx = match dex.intern_field(&class_desc, &field.name, &field.field_type) {
            Ok(idx) => idx,
            Err(_) => return 0,
        };
        let af = wit_to_access_flags(field.access_flags);
        let ef = EncodedField {
            field: field_idx,
            access_flags: af,
        };
        let class = &mut dex.classes[ch.class_idx];
        if af.contains(AccessFlags::STATIC) {
            class.add_static_field(ef);
        } else {
            class.add_instance_field(ef);
        }
        0 // field handles are not tracked in the handle table
    }

    fn remove_field(&mut self, c: u32, name: String) {
        let ch = self.handles.get_class(c).expect("valid class handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(ch.dex_idx).expect("dex");
        let fields_to_remove: Vec<FieldIdx> = dex.classes[ch.class_idx].class_data.as_ref()
            .map(|data| {
                data.static_fields.iter().chain(&data.instance_fields)
                    .filter(|f| dex.string(dex.fields[f.field.0 as usize].name) == name)
                    .map(|f| f.field)
                    .collect()
            })
            .unwrap_or_default();
        if let Some(data) = &mut dex.classes[ch.class_idx].class_data {
            data.static_fields.retain(|f| !fields_to_remove.contains(&f.field));
            data.instance_fields.retain(|f| !fields_to_remove.contains(&f.field));
        }
    }

    fn set_field_access_flags(&mut self, c: u32, field_name: String, flags: types::AccessFlags) {
        let ch = self.handles.get_class(c).expect("valid class handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(ch.dex_idx).expect("dex");
        let af = wit_to_access_flags(flags);
        let target_field = dex.classes[ch.class_idx].class_data.as_ref().and_then(|data| {
            data.static_fields.iter().chain(&data.instance_fields)
                .find(|f| dex.string(dex.fields[f.field.0 as usize].name) == field_name)
                .map(|f| f.field)
        });
        if let Some(field_idx) = target_field {
            if let Some(data) = &mut dex.classes[ch.class_idx].class_data {
                for f in data.static_fields.iter_mut().chain(data.instance_fields.iter_mut()) {
                    if f.field == field_idx {
                        f.access_flags = af;
                        return;
                    }
                }
            }
        }
    }

    fn clone_method(&mut self, m: u32, new_name: Option<String>) -> u32 {
        let mh = self.handles.get_method(m).expect("valid method handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(mh.dex_idx).expect("dex");
        let method = get_method_ref(dex, mh).clone();
        let method_idx = if let Some(name) = new_name {
            let mid = &dex.methods[method.method.0 as usize];
            let class_desc = dex.type_descriptor(mid.class).to_string();
            let proto = &dex.prototypes[mid.proto.0 as usize];
            let ret = dex.type_descriptor(proto.return_type);
            let params: Vec<&str> = proto.parameters.iter().map(|p| dex.type_descriptor(*p)).collect();
            let proto_str = format!("({}){}", params.join(""), ret);
            match dex.intern_method(&class_desc, &name, &proto_str) {
                Ok(idx) => idx,
                Err(_) => return 0,
            }
        } else {
            method.method
        };
        let cloned = EncodedMethod {
            method: method_idx,
            access_flags: method.access_flags,
            code: method.code.clone(),
        };
        let class = &mut dex.classes[mh.class_idx];
        let (mi, is_virtual) = if mh.is_virtual {
            class.add_virtual_method(cloned);
            (class.class_data.as_ref().map(|d| d.virtual_methods.len() - 1).unwrap_or(0), true)
        } else {
            class.add_direct_method(cloned);
            (class.class_data.as_ref().map(|d| d.direct_methods.len() - 1).unwrap_or(0), false)
        };
        self.handles.alloc_method(mh.dex_idx, mh.class_idx, mi, is_virtual)
    }

    fn superclass_chain(&mut self, c: u32) -> Vec<u32> {
        let ch = self.handles.get_class(c).expect("valid class handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file(ch.dex_idx).expect("dex");
        let mut locations = Vec::new();
        let mut current_type = dex.classes[ch.class_idx].superclass;
        while let Some(super_type) = current_type {
            let found = dex.classes.iter().enumerate().find(|(_, cl)| cl.class_type == super_type);
            match found {
                Some((ci, class)) => {
                    locations.push(ci);
                    current_type = class.superclass;
                }
                None => break,
            }
        }
        locations.into_iter().map(|ci| self.handles.alloc_class(ch.dex_idx, ci)).collect()
    }

    fn definal_class(&mut self, c: u32) {
        let ch = self.handles.get_class(c).expect("valid class handle");
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(ch.dex_idx).expect("dex");
        dex.classes[ch.class_idx].access_flags.remove(AccessFlags::FINAL);
        if let Some(data) = &mut dex.classes[ch.class_idx].class_data {
            for m in data.direct_methods.iter_mut().chain(data.virtual_methods.iter_mut()) {
                m.access_flags.remove(AccessFlags::FINAL);
            }
        }
    }

    // ── DEX access ──

    fn dex_count(&mut self) -> u32 {
        self.ctx().dex_count() as u32
    }

    fn dex(&mut self, index: u32) -> u32 {
        index
    }

    fn method_dex(&mut self, m: u32) -> u32 {
        self.handles.get_method(m).expect("valid method handle").dex_idx as u32
    }

    // ── Interning ──

    fn intern_string(&mut self, d: u32, s: String) -> u32 {
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(d as usize).expect("dex");
        dex.intern_string(&s).0
    }

    fn intern_type(&mut self, d: u32, descriptor: String) -> u32 {
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(d as usize).expect("dex");
        dex.intern_type(&descriptor).0
    }

    fn intern_proto(&mut self, d: u32, proto: String) -> u32 {
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(d as usize).expect("dex");
        match dex.intern_proto(&proto) {
            Ok(idx) => idx.0 as u32,
            Err(_) => 0,
        }
    }

    fn intern_method(&mut self, d: u32, class: String, name: String, proto: String) -> u32 {
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(d as usize).expect("dex");
        match dex.intern_method(&class, &name, &proto) {
            Ok(idx) => idx.0,
            Err(_) => 0,
        }
    }

    fn intern_field(&mut self, d: u32, class: String, name: String, field_type: String) -> u32 {
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(d as usize).expect("dex");
        match dex.intern_field(&class, &name, &field_type) {
            Ok(idx) => idx.0,
            Err(_) => 0,
        }
    }

    fn find_string_idx(&mut self, d: u32, s: String) -> Option<u32> {
        let ctx = self.ctx();
        let dex = ctx.dex_file(d as usize).expect("dex");
        dex.find_string_idx(&s).map(|idx| idx.0)
    }

    fn get_string(&mut self, d: u32, idx: u32) -> String {
        let ctx = self.ctx();
        let dex = ctx.dex_file(d as usize).expect("dex");
        dex.string(StringIdx(idx)).to_string()
    }

    fn get_type_descriptor(&mut self, d: u32, idx: u32) -> String {
        let ctx = self.ctx();
        let dex = ctx.dex_file(d as usize).expect("dex");
        use stitch_apk::stitch_dex::TypeIdx;
        dex.type_descriptor(TypeIdx(idx)).to_string()
    }

    fn build_lookups(&mut self, d: u32) {
        let ctx = self.ctx();
        let dex = ctx.dex_file_mut(d as usize).expect("dex");
        dex.build_lookups();
    }

    fn merge_extension_dex(&mut self, paths: Vec<String>) -> u32 {
        let bundle_dir = match &self.bundle_dir {
            Some(dir) => dir.clone(),
            None => return 0,
        };
        let full_paths: Vec<std::path::PathBuf> = paths.iter().map(|p| bundle_dir.join(p)).collect();
        match self.ctx().merge_extension_dex(&full_paths) {
            Ok(count) => count as u32,
            Err(_) => 0,
        }
    }
}

// ── Helper functions ──

fn convert_fingerprint(fp: &types::Fingerprint) -> Fingerprint {
    Fingerprint {
        name: fp.name.clone(),
        defining_class: fp.defining_class.clone(),
        access_flags: fp.access_flags.map(wit_to_access_flags),
        return_type: fp.return_type.clone(),
        parameters: fp.parameters.clone(),
        opcodes: fp.opcodes.as_ref().map(|ops| {
            ops.iter().map(|o| match o {
                None => InstructionPattern::Any,
                Some(op) => InstructionPattern::OpcodeValue(*op),
            }).collect()
        }),
        strings: fp.strings.clone(),
    }
}

fn collect_fields(dex: &DexFile, class: &stitch_apk::stitch_dex::ClassDef, statics: bool, instances: bool) -> Vec<FieldInfo> {
    let mut fields = Vec::new();
    if let Some(data) = &class.class_data {
        let iter: Box<dyn Iterator<Item = &EncodedField>> = match (statics, instances) {
            (true, true) => Box::new(data.static_fields.iter().chain(&data.instance_fields)),
            (true, false) => Box::new(data.static_fields.iter()),
            (false, true) => Box::new(data.instance_fields.iter()),
            (false, false) => return fields,
        };
        for f in iter {
            let field_id = &dex.fields[f.field.0 as usize];
            fields.push(FieldInfo {
                class_descriptor: dex.type_descriptor(class.class_type).to_string(),
                name: dex.string(field_id.name).to_string(),
                field_type: dex.type_descriptor(field_id.type_).to_string(),
                access_flags: access_flags_to_wit(f.access_flags),
            });
        }
    }
    fields
}

fn get_insn_register(state: &WasmState, m: u32, index: u32, reg_pos: usize) -> u16 {
    let mh = state.handles.get_method(m).expect("valid method handle");
    let ctx = state.ctx();
    let dex = ctx.dex_file(mh.dex_idx).expect("dex");
    let method = get_method_ref(dex, mh);
    method.code.as_ref().and_then(|c| {
        c.instructions.get(index as usize).and_then(|insn| {
            let regs = insn.registers_used();
            regs.get(reg_pos).copied()
        })
    }).unwrap_or(0)
}
