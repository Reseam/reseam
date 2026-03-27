use smallvec::SmallVec;
use stitch_apk::stitch_dex::{
    AccessFlags, AnnotationElement as DexAnnotationElement, AnnotationItem as DexAnnotationItem,
    AnnotationVisibility as DexAnnotationVisibility, AnnotationsDirectory,
    CatchHandler as DexCatchHandler, CodeItem, DexFile, EncodedField, EncodedMethod, EncodedValue,
    FieldIdx, Fingerprint, Instruction as DexInsn, InstructionPattern, StringIdx,
    TryItem as DexTryItem, TypedCatch as DexTypedCatch,
};

use boltffi::export;

use super::convert::{dex_to_kotlin, kotlin_to_dex};

#[export]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
use super::types::{
    AnnotationItem, ClassInfo, EncodedVal, FieldInfo, FingerprintDef, FingerprintResult,
    Instruction, InstructionHit, MethodInfo, MethodRef, NewField, NewMethod,
};
use super::{
    find_method_location, get_method_mut, get_method_ref, method_match_location, scan_location,
    with_ctx, with_handles, BUNDLE_DIR,
};

#[export]
pub fn find_method(class_descriptor: String, method_name: String) -> Option<u32> {
    with_ctx(|ctx| {
        let result = ctx.find_method(&class_descriptor, &method_name);
        match result {
            Some((dex_idx, method)) => {
                let (ci, mi, iv) = find_method_location(ctx, dex_idx, method)?;
                Some(with_handles(|h| h.alloc_method(dex_idx, ci, mi, iv)))
            }
            None => None,
        }
    })
}

#[export]
pub fn find_method_by_name(name: String) -> Option<u32> {
    with_ctx(|ctx| {
        let result = ctx.find_method_by_name(&name);
        match result {
            Some((dex_idx, mm)) => {
                let (ci, mi, iv) = method_match_location(ctx, dex_idx, &mm)?;
                Some(with_handles(|h| h.alloc_method(dex_idx, ci, mi, iv)))
            }
            None => None,
        }
    })
}

#[export]
pub fn find_methods_by_strings(strings: Vec<String>) -> Vec<u32> {
    let locations: Vec<(usize, usize, usize, bool)> = with_ctx(|ctx| {
        let str_refs: Vec<&str> = strings.iter().map(|s| s.as_str()).collect();
        let matches = ctx.find_methods_by_strings(&str_refs);
        matches
            .iter()
            .filter_map(|(dex_idx, mm)| {
                let (ci, mi, iv) = method_match_location(ctx, *dex_idx, mm)?;
                Some((*dex_idx, ci, mi, iv))
            })
            .collect()
    });
    locations
        .into_iter()
        .map(|(di, ci, mi, iv)| with_handles(|h| h.alloc_method(di, ci, mi, iv)))
        .collect()
}

#[export]
pub fn find_methods_by_opcodes(pattern: Vec<i32>) -> Vec<u32> {
    let ip: Vec<InstructionPattern> = pattern
        .iter()
        .map(|o| {
            if *o < 0 {
                InstructionPattern::Any
            } else {
                InstructionPattern::OpcodeValue(*o as u16)
            }
        })
        .collect();
    let locations: Vec<(usize, usize, usize, bool)> = with_ctx(|ctx| {
        let matches = ctx.find_methods_with_opcodes(&ip);
        matches
            .iter()
            .filter_map(|(dex_idx, mm)| {
                let (ci, mi, iv) = method_match_location(ctx, *dex_idx, mm)?;
                Some((*dex_idx, ci, mi, iv))
            })
            .collect()
    });
    locations
        .into_iter()
        .map(|(di, ci, mi, iv)| with_handles(|h| h.alloc_method(di, ci, mi, iv)))
        .collect()
}

#[export]
pub fn find_method_by_fingerprint(fp: FingerprintDef) -> Option<FingerprintResult> {
    let dex_fp = convert_fingerprint(&fp);
    let (dex_idx, ci, mi, iv, matched_count) = with_ctx(|ctx| {
        let (dex_idx, fm) = ctx.find_method_by_fingerprint(&dex_fp)?;
        let (ci, mi, iv) = find_method_location(ctx, dex_idx, fm.method)?;
        Some((dex_idx, ci, mi, iv, fm.matched_indices.len() as u32))
    })?;
    let mh = with_handles(|h| h.alloc_method(dex_idx, ci, mi, iv));
    Some(FingerprintResult {
        method: mh,
        matched_count,
    })
}

#[export]
pub fn find_methods_by_fingerprint(fp: FingerprintDef) -> Vec<FingerprintResult> {
    let dex_fp = convert_fingerprint(&fp);
    let locs: Vec<(usize, usize, usize, bool, u32)> = with_ctx(|ctx| {
        let matches = ctx.find_methods_by_fingerprint(&dex_fp);
        matches
            .iter()
            .filter_map(|(dex_idx, fm)| {
                let (ci, mi, iv) = find_method_location(ctx, *dex_idx, fm.method)?;
                Some((*dex_idx, ci, mi, iv, fm.matched_indices.len() as u32))
            })
            .collect()
    });
    locs.into_iter()
        .map(|(di, ci, mi, iv, count)| {
            let mh = with_handles(|h| h.alloc_method(di, ci, mi, iv));
            FingerprintResult {
                method: mh,
                matched_count: count,
            }
        })
        .collect()
}

#[export]
pub fn find_class(descriptor: String) -> Option<u32> {
    with_ctx(|ctx| match ctx.find_class(&descriptor) {
        Some((dex_idx, class)) => {
            let dex = ctx.dex_file(dex_idx)?;
            let class_idx = dex.classes.iter().position(|c| std::ptr::eq(c, class))?;
            Some(with_handles(|h| h.alloc_class(dex_idx, class_idx)))
        }
        None => None,
    })
}

#[export]
pub fn get_method_info(m: u32) -> Option<MethodInfo> {
    with_ctx(|ctx| {
        let mh = with_handles(|h| h.get_method(m))?;
        let dex = ctx.dex_file(mh.dex_idx)?;
        let class = dex.classes.get(mh.class_idx)?;
        let method = get_method_ref(dex, mh);
        let method_id = dex.methods.get(method.method.0 as usize)?;
        let class_desc = dex.type_descriptor(class.class_type).to_string();
        let name = dex.string(method_id.name).to_string();
        let proto_def = dex.prototypes.get(method_id.proto.0 as usize)?;
        let ret = dex.type_descriptor(proto_def.return_type);
        let params: Vec<&str> = proto_def
            .parameters
            .iter()
            .map(|p| dex.type_descriptor(*p))
            .collect();
        let proto = format!("({}){}", params.join(""), ret);
        let (reg_count, ins, outs, insn_count) = match &method.code {
            Some(c) => (
                c.registers_size,
                c.ins_size,
                c.outs_size,
                c.instructions.len() as u32,
            ),
            None => (0, 0, 0, 0),
        };
        Some(MethodInfo {
            class_descriptor: class_desc,
            method_name: name,
            proto,
            access_flags: method.access_flags.bits(),
            dex_index: mh.dex_idx as u32,
            register_count: reg_count,
            ins_size: ins,
            outs_size: outs,
            instruction_count: insn_count,
        })
    })
}

#[export]
pub fn get_class_info(c: u32) -> Option<ClassInfo> {
    with_ctx(|ctx| {
        let ch = with_handles(|h| h.get_class(c))?;
        let dex = ctx.dex_file(ch.dex_idx)?;
        let class = dex.classes.get(ch.class_idx)?;
        let desc = dex.type_descriptor(class.class_type).to_string();
        let superclass = class.superclass.map(|s| dex.type_descriptor(s).to_string());
        let interfaces: Vec<String> = class
            .interfaces
            .iter()
            .map(|i| dex.type_descriptor(*i).to_string())
            .collect();
        let (dm, vm, sf, inf) = match &class.class_data {
            Some(d) => (
                d.direct_methods.len() as u32,
                d.virtual_methods.len() as u32,
                d.static_fields.len() as u32,
                d.instance_fields.len() as u32,
            ),
            None => (0, 0, 0, 0),
        };
        Some(ClassInfo {
            descriptor: desc,
            access_flags: class.access_flags.bits(),
            superclass,
            interfaces,
            dex_index: ch.dex_idx as u32,
            direct_method_count: dm,
            virtual_method_count: vm,
            static_field_count: sf,
            instance_field_count: inf,
        })
    })
}

#[export]
pub fn class_methods(c: u32) -> Vec<u32> {
    let ch = match with_handles(|h| h.get_class(c)) {
        Some(ch) => ch,
        None => return Vec::new(),
    };
    let (dm_count, vm_count) = with_ctx(|ctx| {
        let dex = match ctx.dex_file(ch.dex_idx) {
            Some(d) => d,
            None => return (0, 0),
        };
        match dex
            .classes
            .get(ch.class_idx)
            .and_then(|c| c.class_data.as_ref())
        {
            Some(d) => (d.direct_methods.len(), d.virtual_methods.len()),
            None => (0, 0),
        }
    });
    let mut handles = Vec::with_capacity(dm_count + vm_count);
    for i in 0..dm_count {
        handles.push(with_handles(|h| {
            h.alloc_method(ch.dex_idx, ch.class_idx, i, false)
        }));
    }
    for i in 0..vm_count {
        handles.push(with_handles(|h| {
            h.alloc_method(ch.dex_idx, ch.class_idx, i, true)
        }));
    }
    handles
}

#[export]
pub fn class_direct_methods(c: u32) -> Vec<u32> {
    let ch = match with_handles(|h| h.get_class(c)) {
        Some(ch) => ch,
        None => return Vec::new(),
    };
    let dm_count = with_ctx(|ctx| {
        ctx.dex_file(ch.dex_idx)
            .and_then(|dex| dex.classes.get(ch.class_idx))
            .and_then(|c| c.class_data.as_ref())
            .map(|d| d.direct_methods.len())
            .unwrap_or(0)
    });
    (0..dm_count)
        .map(|i| with_handles(|h| h.alloc_method(ch.dex_idx, ch.class_idx, i, false)))
        .collect()
}

#[export]
pub fn class_virtual_methods(c: u32) -> Vec<u32> {
    let ch = match with_handles(|h| h.get_class(c)) {
        Some(ch) => ch,
        None => return Vec::new(),
    };
    let vm_count = with_ctx(|ctx| {
        ctx.dex_file(ch.dex_idx)
            .and_then(|dex| dex.classes.get(ch.class_idx))
            .and_then(|c| c.class_data.as_ref())
            .map(|d| d.virtual_methods.len())
            .unwrap_or(0)
    });
    (0..vm_count)
        .map(|i| with_handles(|h| h.alloc_method(ch.dex_idx, ch.class_idx, i, true)))
        .collect()
}

#[export]
pub fn class_fields(c: u32) -> Vec<FieldInfo> {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return Vec::new(),
        };
        let dex = match ctx.dex_file(ch.dex_idx) {
            Some(d) => d,
            None => return Vec::new(),
        };
        match dex.classes.get(ch.class_idx) {
            Some(class) => collect_fields(dex, class, true, true),
            None => Vec::new(),
        }
    })
}

#[export]
pub fn class_static_fields(c: u32) -> Vec<FieldInfo> {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return Vec::new(),
        };
        let dex = match ctx.dex_file(ch.dex_idx) {
            Some(d) => d,
            None => return Vec::new(),
        };
        match dex.classes.get(ch.class_idx) {
            Some(class) => collect_fields(dex, class, true, false),
            None => Vec::new(),
        }
    })
}

#[export]
pub fn class_instance_fields(c: u32) -> Vec<FieldInfo> {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return Vec::new(),
        };
        let dex = match ctx.dex_file(ch.dex_idx) {
            Some(d) => d,
            None => return Vec::new(),
        };
        match dex.classes.get(ch.class_idx) {
            Some(class) => collect_fields(dex, class, false, true),
            None => Vec::new(),
        }
    })
}

#[export]
pub fn get_instructions(m: u32) -> Vec<Instruction> {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return Vec::new(),
        };
        let dex = match ctx.dex_file(mh.dex_idx) {
            Some(d) => d,
            None => return Vec::new(),
        };
        let method = get_method_ref(dex, mh);
        match &method.code {
            Some(c) => c
                .instructions
                .iter()
                .map(|insn| dex_to_kotlin(insn, dex))
                .collect(),
            None => Vec::new(),
        }
    })
}

#[export]
pub fn get_instruction(m: u32, index: u32) -> Instruction {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return nop(),
        };
        let dex = match ctx.dex_file(mh.dex_idx) {
            Some(d) => d,
            None => return nop(),
        };
        let method = get_method_ref(dex, mh);
        match &method.code {
            Some(c) => match c.instructions.get(index as usize) {
                Some(insn) => dex_to_kotlin(insn, dex),
                None => nop(),
            },
            None => nop(),
        }
    })
}

#[export]
pub fn instruction_count(m: u32) -> u32 {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return 0,
        };
        let dex = match ctx.dex_file(mh.dex_idx) {
            Some(d) => d,
            None => return 0,
        };
        let method = get_method_ref(dex, mh);
        method
            .code
            .as_ref()
            .map(|c| c.instructions.len() as u32)
            .unwrap_or(0)
    })
}

#[export]
pub fn index_of_first(m: u32, start: u32, op: u16) -> Option<u32> {
    with_ctx(|ctx| {
        let mh = with_handles(|h| h.get_method(m))?;
        let dex = ctx.dex_file(mh.dex_idx)?;
        let method = get_method_ref(dex, mh);
        method.code.as_ref().and_then(|c| {
            c.instructions
                .iter()
                .enumerate()
                .skip(start as usize)
                .find(|(_, insn)| insn.opcode() == Some(op))
                .map(|(i, _)| i as u32)
        })
    })
}

#[export]
pub fn index_of_first_reversed(m: u32, start: u32, op: u16) -> Option<u32> {
    with_ctx(|ctx| {
        let mh = with_handles(|h| h.get_method(m))?;
        let dex = ctx.dex_file(mh.dex_idx)?;
        let method = get_method_ref(dex, mh);
        method.code.as_ref().and_then(|c| {
            let end = (start as usize).min(c.instructions.len());
            c.instructions[..end]
                .iter()
                .enumerate()
                .rev()
                .find(|(_, insn)| insn.opcode() == Some(op))
                .map(|(i, _)| i as u32)
        })
    })
}

#[export]
pub fn index_of_first_literal(m: u32, literal: i64) -> Option<u32> {
    with_ctx(|ctx| {
        let mh = with_handles(|h| h.get_method(m))?;
        let dex = ctx.dex_file(mh.dex_idx)?;
        let method = get_method_ref(dex, mh);
        method.code.as_ref().and_then(|c| {
            c.instructions
                .iter()
                .position(|insn| insn.literal() == Some(literal))
                .map(|i| i as u32)
        })
    })
}

#[export]
pub fn index_of_first_string(m: u32, s: String) -> Option<u32> {
    with_ctx(|ctx| {
        let mh = with_handles(|h| h.get_method(m))?;
        let dex = ctx.dex_file(mh.dex_idx)?;
        let target_idx = dex.find_string_idx(&s)?;
        let method = get_method_ref(dex, mh);
        method.code.as_ref().and_then(|c| {
            c.instructions
                .iter()
                .position(|insn| insn.string_ref() == Some(target_idx))
                .map(|i| i as u32)
        })
    })
}

#[export]
pub fn find_all_indices(m: u32, op: u16) -> Vec<u32> {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return Vec::new(),
        };
        let dex = match ctx.dex_file(mh.dex_idx) {
            Some(d) => d,
            None => return Vec::new(),
        };
        let method = get_method_ref(dex, mh);
        match &method.code {
            Some(c) => c
                .instructions
                .iter()
                .enumerate()
                .filter(|(_, insn)| insn.opcode() == Some(op))
                .map(|(i, _)| i as u32)
                .collect(),
            None => Vec::new(),
        }
    })
}

#[export]
pub fn find_instructions_by_literal(literal: i64) -> Vec<InstructionHit> {
    let locations: Vec<(usize, usize, usize, bool, usize)> = with_ctx(|ctx| {
        let hits = ctx.find_instructions_by_literal(literal);
        hits.iter()
            .filter_map(|loc| {
                let (actual, is_virtual) =
                    scan_location(ctx, loc.dex_idx, loc.class_idx, loc.method_idx)?;
                Some((loc.dex_idx, loc.class_idx, actual, is_virtual, loc.insn_idx))
            })
            .collect()
    });
    locations
        .into_iter()
        .map(|(di, ci, mi, iv, ii)| InstructionHit {
            method: with_handles(|h| h.alloc_method(di, ci, mi, iv)),
            index: ii as u32,
        })
        .collect()
}

#[export]
pub fn find_instructions_by_string(s: String) -> Vec<InstructionHit> {
    let locations: Vec<(usize, usize, usize, bool, usize)> = with_ctx(|ctx| {
        let hits = ctx.find_instructions_by_string(&s);
        hits.iter()
            .filter_map(|loc| {
                let (actual, is_virtual) =
                    scan_location(ctx, loc.dex_idx, loc.class_idx, loc.method_idx)?;
                Some((loc.dex_idx, loc.class_idx, actual, is_virtual, loc.insn_idx))
            })
            .collect()
    });
    locations
        .into_iter()
        .map(|(di, ci, mi, iv, ii)| InstructionHit {
            method: with_handles(|h| h.alloc_method(di, ci, mi, iv)),
            index: ii as u32,
        })
        .collect()
}

#[export]
pub fn find_instructions_by_string_contains(substring: String) -> Vec<InstructionHit> {
    let locations: Vec<(usize, usize, usize, bool, usize)> = with_ctx(|ctx| {
        let hits = ctx.find_instructions_by_string_contains(&substring);
        hits.iter()
            .filter_map(|loc| {
                let (actual, is_virtual) =
                    scan_location(ctx, loc.dex_idx, loc.class_idx, loc.method_idx)?;
                Some((loc.dex_idx, loc.class_idx, actual, is_virtual, loc.insn_idx))
            })
            .collect()
    });
    locations
        .into_iter()
        .map(|(di, ci, mi, iv, ii)| InstructionHit {
            method: with_handles(|h| h.alloc_method(di, ci, mi, iv)),
            index: ii as u32,
        })
        .collect()
}

#[export]
pub fn find_instructions_by_resource_id(res_type: String, res_name: String) -> Vec<InstructionHit> {
    let res_id = match with_ctx(|ctx| ctx.find_resource_id(&res_type, &res_name)) {
        Some(id) => id,
        None => return Vec::new(),
    };
    find_instructions_by_literal(res_id as i64)
}

#[export]
pub fn set_instructions(m: u32, insns: Vec<Instruction>) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let dex_insns: Vec<DexInsn> = insns.iter().map(|ki| kotlin_to_dex(ki, dex)).collect();
        let method = get_method_mut(dex, mh);
        if let Some(code) = &mut method.code {
            code.set_instructions(dex_insns);
        }
    });
}

#[export]
pub fn insert_instruction(m: u32, index: u32, insn: Instruction) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let dex_insn = kotlin_to_dex(&insn, dex);
        let method = get_method_mut(dex, mh);
        if let Some(code) = &mut method.code {
            code.insert_instruction(index as usize, dex_insn);
        }
    });
}

#[export]
pub fn insert_instructions(m: u32, index: u32, insns: Vec<Instruction>) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let dex_insns: Vec<DexInsn> = insns.iter().map(|ki| kotlin_to_dex(ki, dex)).collect();
        let method = get_method_mut(dex, mh);
        if let Some(code) = &mut method.code {
            code.insert_instructions(index as usize, &dex_insns);
        }
    });
}

#[export]
pub fn replace_instruction(m: u32, index: u32, insn: Instruction) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let dex_insn = kotlin_to_dex(&insn, dex);
        let method = get_method_mut(dex, mh);
        if let Some(code) = &mut method.code {
            code.replace_instruction(index as usize, dex_insn);
        }
    });
}

#[export]
pub fn remove_instruction(m: u32, index: u32) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let method = get_method_mut(dex, mh);
        if let Some(code) = &mut method.code {
            code.remove_instruction(index as usize);
        }
    });
}

#[export]
pub fn remove_instructions(m: u32, index: u32, count: u32) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let method = get_method_mut(dex, mh);
        if let Some(code) = &mut method.code {
            for _ in 0..count {
                if (index as usize) < code.instructions.len() {
                    code.remove_instruction(index as usize);
                }
            }
        }
    });
}

#[export]
pub fn return_early(m: u32) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let method = get_method_mut(dex, mh);
        method.return_early();
    });
}

#[export]
pub fn return_early_int(m: u32, value: i32) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let method = get_method_mut(dex, mh);
        method.return_early_int(value);
    });
}

#[export]
pub fn set_registers(m: u32, registers_size: u16, outs_size: u16) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let method = get_method_mut(dex, mh);
        if let Some(code) = &mut method.code {
            code.registers_size = registers_size;
            code.outs_size = outs_size;
        }
    });
}

#[export]
pub fn registers_size(m: u32) -> u16 {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return 0,
        };
        let dex = match ctx.dex_file(mh.dex_idx) {
            Some(d) => d,
            None => return 0,
        };
        get_method_ref(dex, mh)
            .code
            .as_ref()
            .map(|c| c.registers_size)
            .unwrap_or(0)
    })
}

#[export]
pub fn ins_size(m: u32) -> u16 {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return 0,
        };
        let dex = match ctx.dex_file(mh.dex_idx) {
            Some(d) => d,
            None => return 0,
        };
        get_method_ref(dex, mh)
            .code
            .as_ref()
            .map(|c| c.ins_size)
            .unwrap_or(0)
    })
}

#[export]
pub fn outs_size(m: u32) -> u16 {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return 0,
        };
        let dex = match ctx.dex_file(mh.dex_idx) {
            Some(d) => d,
            None => return 0,
        };
        get_method_ref(dex, mh)
            .code
            .as_ref()
            .map(|c| c.outs_size)
            .unwrap_or(0)
    })
}

#[export]
pub fn find_free_register(m: u32, at_index: u32, exclude: Vec<u16>) -> u16 {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return 0,
        };
        let dex = match ctx.dex_file(mh.dex_idx) {
            Some(d) => d,
            None => return 0,
        };
        let method = get_method_ref(dex, mh);
        method
            .code
            .as_ref()
            .and_then(|c| ctx.find_free_register(c, at_index as usize, &exclude))
            .unwrap_or(0)
    })
}

#[export]
pub fn find_free_registers(m: u32, at_index: u32, count: u32, exclude: Vec<u16>) -> Vec<u16> {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return Vec::new(),
        };
        let dex = match ctx.dex_file(mh.dex_idx) {
            Some(d) => d,
            None => return Vec::new(),
        };
        let method = get_method_ref(dex, mh);
        method
            .code
            .as_ref()
            .and_then(|c| ctx.find_free_registers(c, at_index as usize, count as usize, &exclude))
            .unwrap_or_default()
    })
}

#[export]
pub fn instruction_register_a(m: u32, index: u32) -> u16 {
    get_insn_register(m, index, 0)
}

#[export]
pub fn instruction_register_b(m: u32, index: u32) -> u16 {
    get_insn_register(m, index, 1)
}

#[export]
pub fn instruction_register_c(m: u32, index: u32) -> u16 {
    get_insn_register(m, index, 2)
}

#[export]
pub fn instruction_register_d(m: u32, index: u32) -> u16 {
    get_insn_register(m, index, 3)
}

#[export]
pub fn instruction_wide_literal(m: u32, index: u32) -> i64 {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return 0,
        };
        let dex = match ctx.dex_file(mh.dex_idx) {
            Some(d) => d,
            None => return 0,
        };
        let method = get_method_ref(dex, mh);
        method
            .code
            .as_ref()
            .and_then(|c| {
                c.instructions
                    .get(index as usize)
                    .and_then(|insn| insn.literal())
            })
            .unwrap_or(0)
    })
}

#[export]
pub fn instruction_string_ref(m: u32, index: u32) -> Option<String> {
    with_ctx(|ctx| {
        let mh = with_handles(|h| h.get_method(m))?;
        let dex = ctx.dex_file(mh.dex_idx)?;
        let method = get_method_ref(dex, mh);
        method.code.as_ref().and_then(|c| {
            c.instructions
                .get(index as usize)
                .and_then(|insn| insn.string_ref().map(|idx| dex.string(idx).to_string()))
        })
    })
}

#[export]
pub fn instruction_method_ref(m: u32, index: u32) -> Option<MethodRef> {
    with_ctx(|ctx| {
        let mh = with_handles(|h| h.get_method(m))?;
        let dex = ctx.dex_file(mh.dex_idx)?;
        let method = get_method_ref(dex, mh);
        method.code.as_ref().and_then(|c| {
            c.instructions.get(index as usize).and_then(|insn| {
                insn.method_ref().map(|method_idx| {
                    let mid = &dex.methods[method_idx.0 as usize];
                    let class = dex.type_descriptor(mid.class).to_string();
                    let name = dex.string(mid.name).to_string();
                    let proto = &dex.prototypes[mid.proto.0 as usize];
                    let ret = dex.type_descriptor(proto.return_type);
                    let params: Vec<&str> = proto
                        .parameters
                        .iter()
                        .map(|p| dex.type_descriptor(*p))
                        .collect();
                    MethodRef {
                        defining_class: class,
                        name,
                        proto: format!("({}){}", params.join(""), ret),
                    }
                })
            })
        })
    })
}

#[export]
pub fn instruction_field_ref(m: u32, index: u32) -> Option<super::types::FieldRef> {
    with_ctx(|ctx| {
        let mh = with_handles(|h| h.get_method(m))?;
        let dex = ctx.dex_file(mh.dex_idx)?;
        let method = get_method_ref(dex, mh);
        method.code.as_ref().and_then(|c| {
            c.instructions.get(index as usize).and_then(|insn| {
                insn.field_ref().map(|field_idx| {
                    let fid = &dex.fields[field_idx.0 as usize];
                    super::types::FieldRef {
                        defining_class: dex.type_descriptor(fid.class).to_string(),
                        name: dex.string(fid.name).to_string(),
                        field_type: dex.type_descriptor(fid.type_).to_string(),
                    }
                })
            })
        })
    })
}

#[export]
pub fn instruction_type_ref(m: u32, index: u32) -> Option<String> {
    with_ctx(|ctx| {
        let mh = with_handles(|h| h.get_method(m))?;
        let dex = ctx.dex_file(mh.dex_idx)?;
        let method = get_method_ref(dex, mh);
        method.code.as_ref().and_then(|c| {
            c.instructions.get(index as usize).and_then(|insn| {
                insn.type_ref()
                    .map(|type_idx| dex.type_descriptor(type_idx).to_string())
            })
        })
    })
}

#[export]
pub fn set_class_access_flags(c: u32, flags: u32) {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return,
        };
        let dex = match ctx.dex_file_mut(ch.dex_idx) {
            Some(d) => d,
            None => return,
        };
        dex.classes[ch.class_idx].access_flags = AccessFlags::from_bits_truncate(flags);
    });
}

#[export]
pub fn set_superclass(c: u32, superclass: String) {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return,
        };
        let dex = match ctx.dex_file_mut(ch.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let _ = dex.set_superclass(ch.class_idx, &superclass);
    });
}

#[export]
pub fn add_interface(c: u32, interface_descriptor: String) {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return,
        };
        let dex = match ctx.dex_file_mut(ch.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let type_idx = dex.intern_type(&interface_descriptor);
        dex.classes[ch.class_idx].interfaces.push(type_idx);
    });
}

#[export]
pub fn remove_class(c: u32) {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return,
        };
        let dex = match ctx.dex_file_mut(ch.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let class_type = dex.classes[ch.class_idx].class_type;
        dex.remove_class(class_type);
    });
}

#[export]
pub fn create_class(dex_index: u32, descriptor: String, flags: u32, superclass: String) -> u32 {
    with_ctx(|ctx| {
        let dex = match ctx.dex_file_mut(dex_index as usize) {
            Some(d) => d,
            None => return 0,
        };
        let af = AccessFlags::from_bits_truncate(flags);
        match dex.create_class(&descriptor, af, Some(&superclass)) {
            Ok(class_idx) => with_handles(|h| h.alloc_class(dex_index as usize, class_idx)),
            Err(_) => 0,
        }
    })
}

#[export]
pub fn add_method(c: u32, method: NewMethod) -> u32 {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return 0,
        };
        let dex = match ctx.dex_file_mut(ch.dex_idx) {
            Some(d) => d,
            None => return 0,
        };
        let class_desc = dex
            .type_descriptor(dex.classes[ch.class_idx].class_type)
            .to_string();
        let method_idx = match dex.intern_method(&class_desc, &method.name, &method.proto) {
            Ok(idx) => idx,
            Err(_) => return 0,
        };
        let af = AccessFlags::from_bits_truncate(method.access_flags);
        let code = if af.contains(AccessFlags::NATIVE) || af.contains(AccessFlags::ABSTRACT) {
            None
        } else {
            let insns: Vec<DexInsn> = method
                .instructions
                .iter()
                .map(|ki| kotlin_to_dex(ki, dex))
                .collect();
            let tries: Vec<DexTryItem> = method
                .tries
                .iter()
                .map(|t| DexTryItem {
                    start_addr: t.start_addr,
                    insn_count: t.insn_count,
                    handler_idx: t.handler_idx as usize,
                })
                .collect();
            let catch_handlers: Vec<DexCatchHandler> = method
                .catch_handlers
                .iter()
                .map(|ch| {
                    let typed_catches = ch
                        .typed_catches
                        .iter()
                        .map(|tc| {
                            let type_idx = dex.intern_type(&tc.exception_type);
                            DexTypedCatch {
                                exception_type: type_idx,
                                addr: tc.addr,
                            }
                        })
                        .collect();
                    DexCatchHandler {
                        typed_catches,
                        catch_all_addr: ch.catch_all_addr,
                    }
                })
                .collect();
            Some(CodeItem {
                registers_size: method.registers_size,
                ins_size: method.ins_size,
                outs_size: method.outs_size,
                debug_info: None,
                instructions: insns,
                tries,
                catch_handlers,
            })
        };
        let em = EncodedMethod {
            method: method_idx,
            access_flags: af,
            code,
        };
        let is_virtual = !af.contains(AccessFlags::STATIC)
            && !af.contains(AccessFlags::CONSTRUCTOR)
            && !af.intersects(AccessFlags::PRIVATE);
        let class = &mut dex.classes[ch.class_idx];
        let mi = if is_virtual {
            class.add_virtual_method(em);
            class
                .class_data
                .as_ref()
                .map(|d| d.virtual_methods.len() - 1)
                .unwrap_or(0)
        } else {
            class.add_direct_method(em);
            class
                .class_data
                .as_ref()
                .map(|d| d.direct_methods.len() - 1)
                .unwrap_or(0)
        };
        with_handles(|h| h.alloc_method(ch.dex_idx, ch.class_idx, mi, is_virtual))
    })
}

#[export]
pub fn remove_method(m: u32) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
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
    });
}

#[export]
pub fn set_method_access_flags(m: u32, flags: u32) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let method = get_method_mut(dex, mh);
        method.access_flags = AccessFlags::from_bits_truncate(flags);
    });
}

#[export]
pub fn add_field(c: u32, field: NewField) -> u32 {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return 0,
        };
        let dex = match ctx.dex_file_mut(ch.dex_idx) {
            Some(d) => d,
            None => return 0,
        };
        let class_desc = dex
            .type_descriptor(dex.classes[ch.class_idx].class_type)
            .to_string();
        let field_idx = match dex.intern_field(&class_desc, &field.name, &field.field_type) {
            Ok(idx) => idx,
            Err(_) => return 0,
        };
        let af = AccessFlags::from_bits_truncate(field.access_flags);
        let ef = EncodedField {
            field: field_idx,
            access_flags: af,
        };
        let init_val = field
            .initial_value
            .as_ref()
            .map(|v| encoded_val_to_dex(v, dex));
        let class = &mut dex.classes[ch.class_idx];
        if af.contains(AccessFlags::STATIC) {
            class.add_static_field(ef);
            if let Some(val) = init_val {
                let static_field_count = class
                    .class_data
                    .as_ref()
                    .map(|d| d.static_fields.len())
                    .unwrap_or(0);
                while class.static_values.len() < static_field_count - 1 {
                    class.static_values.push(EncodedValue::Null);
                }
                class.static_values.push(val);
            }
        } else {
            class.add_instance_field(ef);
        }
        0
    })
}

#[export]
pub fn remove_field(c: u32, name: String) {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return,
        };
        let dex = match ctx.dex_file_mut(ch.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let fields_to_remove: Vec<FieldIdx> = dex.classes[ch.class_idx]
            .class_data
            .as_ref()
            .map(|data| {
                data.static_fields
                    .iter()
                    .chain(&data.instance_fields)
                    .filter(|f| dex.string(dex.fields[f.field.0 as usize].name) == name)
                    .map(|f| f.field)
                    .collect()
            })
            .unwrap_or_default();
        if let Some(data) = &mut dex.classes[ch.class_idx].class_data {
            data.static_fields
                .retain(|f| !fields_to_remove.contains(&f.field));
            data.instance_fields
                .retain(|f| !fields_to_remove.contains(&f.field));
        }
    });
}

#[export]
pub fn set_field_access_flags(c: u32, field_name: String, flags: u32) {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return,
        };
        let dex = match ctx.dex_file_mut(ch.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let af = AccessFlags::from_bits_truncate(flags);
        let target_field = dex.classes[ch.class_idx]
            .class_data
            .as_ref()
            .and_then(|data| {
                data.static_fields
                    .iter()
                    .chain(&data.instance_fields)
                    .find(|f| dex.string(dex.fields[f.field.0 as usize].name) == field_name)
                    .map(|f| f.field)
            });
        if let Some(field_idx) = target_field {
            if let Some(data) = &mut dex.classes[ch.class_idx].class_data {
                for f in data
                    .static_fields
                    .iter_mut()
                    .chain(data.instance_fields.iter_mut())
                {
                    if f.field == field_idx {
                        f.access_flags = af;
                        return;
                    }
                }
            }
        }
    });
}

#[export]
pub fn clone_method(m: u32, new_name: Option<String>) -> u32 {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return 0,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return 0,
        };
        let method = get_method_ref(dex, mh).clone();
        let method_idx = if let Some(name) = new_name {
            let mid = &dex.methods[method.method.0 as usize];
            let class_desc = dex.type_descriptor(mid.class).to_string();
            let proto = &dex.prototypes[mid.proto.0 as usize];
            let ret = dex.type_descriptor(proto.return_type);
            let params: Vec<&str> = proto
                .parameters
                .iter()
                .map(|p| dex.type_descriptor(*p))
                .collect();
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
            (
                class
                    .class_data
                    .as_ref()
                    .map(|d| d.virtual_methods.len() - 1)
                    .unwrap_or(0),
                true,
            )
        } else {
            class.add_direct_method(cloned);
            (
                class
                    .class_data
                    .as_ref()
                    .map(|d| d.direct_methods.len() - 1)
                    .unwrap_or(0),
                false,
            )
        };
        with_handles(|h| h.alloc_method(mh.dex_idx, mh.class_idx, mi, is_virtual))
    })
}

#[export]
pub fn superclass_chain(c: u32) -> Vec<u32> {
    let ch = match with_handles(|h| h.get_class(c)) {
        Some(ch) => ch,
        None => return Vec::new(),
    };
    let locations: Vec<usize> = with_ctx(|ctx| {
        let dex = match ctx.dex_file(ch.dex_idx) {
            Some(d) => d,
            None => return Vec::new(),
        };
        let mut locs = Vec::new();
        let mut current_type = dex.classes[ch.class_idx].superclass;
        while let Some(super_type) = current_type {
            let found = dex
                .classes
                .iter()
                .enumerate()
                .find(|(_, cl)| cl.class_type == super_type);
            match found {
                Some((ci, class)) => {
                    locs.push(ci);
                    current_type = class.superclass;
                }
                None => break,
            }
        }
        locs
    });
    locations
        .into_iter()
        .map(|ci| with_handles(|h| h.alloc_class(ch.dex_idx, ci)))
        .collect()
}

#[export]
pub fn definal_class(c: u32) {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return,
        };
        let dex = match ctx.dex_file_mut(ch.dex_idx) {
            Some(d) => d,
            None => return,
        };
        dex.classes[ch.class_idx]
            .access_flags
            .remove(AccessFlags::FINAL);
        if let Some(data) = &mut dex.classes[ch.class_idx].class_data {
            for m in data
                .direct_methods
                .iter_mut()
                .chain(data.virtual_methods.iter_mut())
            {
                m.access_flags.remove(AccessFlags::FINAL);
            }
        }
    });
}

#[export]
pub fn dex_count() -> u32 {
    with_ctx(|ctx| ctx.dex_count() as u32)
}

#[export]
pub fn method_dex(m: u32) -> u32 {
    with_handles(|h| h.get_method(m))
        .map(|mh| mh.dex_idx as u32)
        .unwrap_or(0)
}

#[export]
pub fn intern_string(d: u32, s: String) -> u32 {
    with_ctx(|ctx| {
        let dex = match ctx.dex_file_mut(d as usize) {
            Some(d) => d,
            None => return 0,
        };
        dex.intern_string(&s).0
    })
}

#[export]
pub fn intern_type(d: u32, descriptor: String) -> u32 {
    with_ctx(|ctx| {
        let dex = match ctx.dex_file_mut(d as usize) {
            Some(d) => d,
            None => return 0,
        };
        dex.intern_type(&descriptor).0
    })
}

#[export]
pub fn intern_proto(d: u32, proto: String) -> u32 {
    with_ctx(|ctx| {
        let dex = match ctx.dex_file_mut(d as usize) {
            Some(d) => d,
            None => return 0,
        };
        match dex.intern_proto(&proto) {
            Ok(idx) => idx.0 as u32,
            Err(_) => 0,
        }
    })
}

#[export]
pub fn intern_method(d: u32, descriptor: String, name: String, proto: String) -> u32 {
    with_ctx(|ctx| {
        let dex = match ctx.dex_file_mut(d as usize) {
            Some(d) => d,
            None => return 0,
        };
        match dex.intern_method(&descriptor, &name, &proto) {
            Ok(idx) => idx.0,
            Err(_) => 0,
        }
    })
}

#[export]
pub fn intern_field(d: u32, descriptor: String, name: String, field_type: String) -> u32 {
    with_ctx(|ctx| {
        let dex = match ctx.dex_file_mut(d as usize) {
            Some(d) => d,
            None => return 0,
        };
        match dex.intern_field(&descriptor, &name, &field_type) {
            Ok(idx) => idx.0,
            Err(_) => 0,
        }
    })
}

#[export]
pub fn find_string_idx(d: u32, s: String) -> Option<u32> {
    with_ctx(|ctx| {
        let dex = ctx.dex_file(d as usize)?;
        dex.find_string_idx(&s).map(|idx| idx.0)
    })
}

#[export]
pub fn get_string(d: u32, idx: u32) -> String {
    with_ctx(|ctx| {
        let dex = match ctx.dex_file(d as usize) {
            Some(d) => d,
            None => return String::new(),
        };
        dex.string(StringIdx(idx)).to_string()
    })
}

#[export]
pub fn get_type_descriptor(d: u32, idx: u32) -> String {
    with_ctx(|ctx| {
        let dex = match ctx.dex_file(d as usize) {
            Some(d) => d,
            None => return String::new(),
        };
        use stitch_apk::stitch_dex::TypeIdx;
        dex.type_descriptor(TypeIdx(idx)).to_string()
    })
}

#[export]
pub fn build_lookups(d: u32) {
    with_ctx(|ctx| {
        if let Some(dex) = ctx.dex_file_mut(d as usize) {
            dex.build_lookups();
        }
    });
}

#[export]
pub fn merge_extension_dex(paths: Vec<String>) -> u32 {
    let bundle_dir = BUNDLE_DIR.with(|bd| bd.borrow().clone());
    let bundle_dir = match bundle_dir {
        Some(dir) => dir,
        None => return 0,
    };
    let full_paths: Vec<std::path::PathBuf> = paths.iter().map(|p| bundle_dir.join(p)).collect();
    with_ctx(|ctx| match ctx.merge_extension_dex(&full_paths) {
        Ok(count) => count as u32,
        Err(_) => 0,
    })
}

#[export]
pub fn add_class_annotation(c: u32, annotation: AnnotationItem) {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return,
        };
        let dex = match ctx.dex_file_mut(ch.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let ann = annotation_to_dex(&annotation, dex);
        let class = &mut dex.classes[ch.class_idx];
        let dir = class
            .annotations
            .get_or_insert_with(|| AnnotationsDirectory {
                class_annotations: Vec::new(),
                field_annotations: Vec::new(),
                method_annotations: Vec::new(),
                parameter_annotations: Vec::new(),
            });
        dir.class_annotations.push(ann);
    });
}

#[export]
pub fn add_method_annotation(m: u32, annotation: AnnotationItem) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let method_idx = get_method_ref(dex, mh).method;
        let ann = annotation_to_dex(&annotation, dex);
        let class = &mut dex.classes[mh.class_idx];
        let dir = class
            .annotations
            .get_or_insert_with(|| AnnotationsDirectory {
                class_annotations: Vec::new(),
                field_annotations: Vec::new(),
                method_annotations: Vec::new(),
                parameter_annotations: Vec::new(),
            });
        if let Some(entry) = dir
            .method_annotations
            .iter_mut()
            .find(|(mid, _)| *mid == method_idx)
        {
            entry.1.push(ann);
        } else {
            dir.method_annotations.push((method_idx, vec![ann]));
        }
    });
}

#[export]
pub fn add_field_annotation(c: u32, field_name: String, annotation: AnnotationItem) {
    with_ctx(|ctx| {
        let ch = match with_handles(|h| h.get_class(c)) {
            Some(ch) => ch,
            None => return,
        };
        let dex = match ctx.dex_file_mut(ch.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let target_field = dex.classes[ch.class_idx]
            .class_data
            .as_ref()
            .and_then(|data| {
                data.static_fields
                    .iter()
                    .chain(&data.instance_fields)
                    .find(|f| dex.string(dex.fields[f.field.0 as usize].name) == field_name)
                    .map(|f| f.field)
            });
        if let Some(field_idx) = target_field {
            let ann = annotation_to_dex(&annotation, dex);
            let class = &mut dex.classes[ch.class_idx];
            let dir = class
                .annotations
                .get_or_insert_with(|| AnnotationsDirectory {
                    class_annotations: Vec::new(),
                    field_annotations: Vec::new(),
                    method_annotations: Vec::new(),
                    parameter_annotations: Vec::new(),
                });
            if let Some(entry) = dir
                .field_annotations
                .iter_mut()
                .find(|(fid, _)| *fid == field_idx)
            {
                entry.1.push(ann);
            } else {
                dir.field_annotations.push((field_idx, vec![ann]));
            }
        }
    });
}

#[export]
pub fn replace_string(m: u32, old: String, new: String) -> bool {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return false,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return false,
        };
        let old_idx = match dex.find_string_idx(&old) {
            Some(idx) => idx,
            None => return false,
        };
        let new_idx = dex.intern_string(&new);
        let method = get_method_mut(dex, mh);
        let code = match &mut method.code {
            Some(c) => c,
            None => return false,
        };
        for insn in &mut code.instructions {
            if insn.string_ref() == Some(old_idx) {
                match insn {
                    DexInsn::ConstString { string, .. }
                    | DexInsn::ConstStringJumbo { string, .. } => {
                        *string = new_idx;
                        return true;
                    }
                    _ => {}
                }
            }
        }
        false
    })
}

#[export]
pub fn replace_all_strings(m: u32, old: String, new: String) -> u32 {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return 0,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return 0,
        };
        let old_idx = match dex.find_string_idx(&old) {
            Some(idx) => idx,
            None => return 0,
        };
        let new_idx = dex.intern_string(&new);
        let method = get_method_mut(dex, mh);
        let code = match &mut method.code {
            Some(c) => c,
            None => return 0,
        };
        let mut count = 0u32;
        for insn in &mut code.instructions {
            if insn.string_ref() == Some(old_idx) {
                match insn {
                    DexInsn::ConstString { string, .. }
                    | DexInsn::ConstStringJumbo { string, .. } => {
                        *string = new_idx;
                        count += 1;
                    }
                    _ => {}
                }
            }
        }
        count
    })
}

#[export]
pub fn replace_literal(m: u32, old: i64, new: i64) -> bool {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return false,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return false,
        };
        let method = get_method_mut(dex, mh);
        let code = match &mut method.code {
            Some(c) => c,
            None => return false,
        };
        for insn in &mut code.instructions {
            if insn.literal() == Some(old) {
                set_insn_literal(insn, new);
                return true;
            }
        }
        false
    })
}

#[export]
pub fn replace_all_literals(m: u32, old: i64, new: i64) -> u32 {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return 0,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return 0,
        };
        let method = get_method_mut(dex, mh);
        let code = match &mut method.code {
            Some(c) => c,
            None => return 0,
        };
        let mut count = 0u32;
        for insn in &mut code.instructions {
            if insn.literal() == Some(old) {
                set_insn_literal(insn, new);
                count += 1;
            }
        }
        count
    })
}

#[export]
pub fn replace_method_call(
    m: u32,
    old_class: String,
    old_name: String,
    new_class: String,
    new_name: String,
    new_proto: String,
) -> u32 {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return 0,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return 0,
        };
        let old_targets: Vec<stitch_apk::stitch_dex::MethodIdx> = dex
            .methods
            .iter()
            .enumerate()
            .filter(|(_, mid)| {
                dex.type_descriptor(mid.class) == old_class
                    && dex.string(mid.name) == old_name
            })
            .map(|(i, _)| stitch_apk::stitch_dex::MethodIdx(i as u32))
            .collect();
        if old_targets.is_empty() {
            return 0;
        }
        let new_method_idx = match dex.intern_method(&new_class, &new_name, &new_proto) {
            Ok(idx) => idx,
            Err(_) => return 0,
        };
        let method = get_method_mut(dex, mh);
        let code = match &mut method.code {
            Some(c) => c,
            None => return 0,
        };
        let mut count = 0u32;
        for insn in &mut code.instructions {
            if let Some(mr) = insn.method_ref() {
                if old_targets.contains(&mr) {
                    set_insn_method_ref(insn, new_method_idx);
                    count += 1;
                }
            }
        }
        count
    })
}

#[export]
pub fn return_early_bool(m: u32, value: bool) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let method = get_method_mut(dex, mh);
        method.return_early_int(if value { 1 } else { 0 });
    });
}

#[export]
pub fn return_early_object_null(m: u32) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let method = get_method_mut(dex, mh);
        method.return_early_object(0);
    });
}

#[export]
pub fn return_early_wide(m: u32, value: i64) {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return,
        };
        let method = get_method_mut(dex, mh);
        method.return_early_wide(value);
    });
}

#[export]
pub fn insert_invoke_static(
    m: u32,
    index: u32,
    class_name: String,
    name: String,
    proto: String,
    registers: Vec<u16>,
) -> bool {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return false,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return false,
        };
        let method_idx = match dex.intern_method(&class_name, &name, &proto) {
            Ok(idx) => idx,
            Err(_) => return false,
        };
        let needs_range = registers.len() > 5
            || registers.iter().any(|&r| r > 15);
        let insn = if needs_range {
            DexInsn::InvokeStaticRange {
                method: method_idx,
                first_reg: registers.first().copied().unwrap_or(0),
                count: registers.len() as u8,
            }
        } else {
            let regs: SmallVec<[u8; 5]> = registers.iter().map(|r| *r as u8).collect();
            DexInsn::InvokeStatic {
                method: method_idx,
                args: regs,
            }
        };
        let method = get_method_mut(dex, mh);
        if let Some(code) = &mut method.code {
            code.insert_instruction(index as usize, insn);
            return true;
        }
        false
    })
}

#[export]
pub fn insert_invoke_static_with_move_result(
    m: u32,
    index: u32,
    class_name: String,
    name: String,
    proto: String,
    registers: Vec<u16>,
    result_register: u16,
    is_object: bool,
) -> bool {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return false,
        };
        let dex = match ctx.dex_file_mut(mh.dex_idx) {
            Some(d) => d,
            None => return false,
        };
        let method_idx = match dex.intern_method(&class_name, &name, &proto) {
            Ok(idx) => idx,
            Err(_) => return false,
        };
        let needs_range = registers.len() > 5
            || registers.iter().any(|&r| r > 15);
        let invoke = if needs_range {
            DexInsn::InvokeStaticRange {
                method: method_idx,
                first_reg: registers.first().copied().unwrap_or(0),
                count: registers.len() as u8,
            }
        } else {
            let regs: SmallVec<[u8; 5]> = registers.iter().map(|r| *r as u8).collect();
            DexInsn::InvokeStatic {
                method: method_idx,
                args: regs,
            }
        };
        let move_result = if is_object {
            DexInsn::MoveResultObject {
                dest: result_register as u8,
            }
        } else {
            DexInsn::MoveResult {
                dest: result_register as u8,
            }
        };
        let method = get_method_mut(dex, mh);
        if let Some(code) = &mut method.code {
            code.insert_instructions(index as usize, &[invoke, move_result]);
            return true;
        }
        false
    })
}

fn set_insn_literal(insn: &mut DexInsn, value: i64) {
    match insn {
        DexInsn::Const4 { value: v, .. } => *v = value as i8,
        DexInsn::Const16 { value: v, .. } => *v = value as i16,
        DexInsn::Const { value: v, .. } => *v = value as i32,
        DexInsn::ConstHigh16 { value: v, .. } => *v = value as i16,
        DexInsn::ConstWide16 { value: v, .. } => *v = value as i16,
        DexInsn::ConstWide32 { value: v, .. } => *v = value as i32,
        DexInsn::ConstWide { value: v, .. } => *v = value,
        DexInsn::ConstWideHigh16 { value: v, .. } => *v = value as i16,
        DexInsn::AddIntLit16 { literal, .. }
        | DexInsn::RsubIntLit16 { literal, .. }
        | DexInsn::MulIntLit16 { literal, .. }
        | DexInsn::DivIntLit16 { literal, .. }
        | DexInsn::RemIntLit16 { literal, .. }
        | DexInsn::AndIntLit16 { literal, .. }
        | DexInsn::OrIntLit16 { literal, .. }
        | DexInsn::XorIntLit16 { literal, .. } => *literal = value as i16,
        DexInsn::AddIntLit8 { literal, .. }
        | DexInsn::RsubIntLit8 { literal, .. }
        | DexInsn::MulIntLit8 { literal, .. }
        | DexInsn::DivIntLit8 { literal, .. }
        | DexInsn::RemIntLit8 { literal, .. }
        | DexInsn::AndIntLit8 { literal, .. }
        | DexInsn::OrIntLit8 { literal, .. }
        | DexInsn::XorIntLit8 { literal, .. }
        | DexInsn::ShlIntLit8 { literal, .. }
        | DexInsn::ShrIntLit8 { literal, .. }
        | DexInsn::UshrIntLit8 { literal, .. } => *literal = value as i8,
        _ => {}
    }
}

fn set_insn_method_ref(insn: &mut DexInsn, new_idx: stitch_apk::stitch_dex::MethodIdx) {
    match insn {
        DexInsn::InvokeVirtual { method, .. }
        | DexInsn::InvokeSuper { method, .. }
        | DexInsn::InvokeDirect { method, .. }
        | DexInsn::InvokeStatic { method, .. }
        | DexInsn::InvokeInterface { method, .. }
        | DexInsn::InvokeVirtualRange { method, .. }
        | DexInsn::InvokeSuperRange { method, .. }
        | DexInsn::InvokeDirectRange { method, .. }
        | DexInsn::InvokeStaticRange { method, .. }
        | DexInsn::InvokeInterfaceRange { method, .. } => {
            *method = new_idx;
        }
        _ => {}
    }
}

fn nop() -> Instruction {
    Instruction::Simple(super::types::SimpleInsn { opcode: 0x00 })
}

fn encoded_val_to_dex(val: &EncodedVal, dex: &mut DexFile) -> EncodedValue {
    match val {
        EncodedVal::Null => EncodedValue::Null,
        EncodedVal::BoolVal(b) => EncodedValue::Boolean(*b),
        EncodedVal::ByteVal(v) => EncodedValue::Byte(*v),
        EncodedVal::ShortVal(v) => EncodedValue::Short(*v),
        EncodedVal::CharVal(v) => EncodedValue::Char(*v),
        EncodedVal::IntVal(v) => EncodedValue::Int(*v),
        EncodedVal::LongVal(v) => EncodedValue::Long(*v),
        EncodedVal::FloatVal(v) => EncodedValue::Float(*v),
        EncodedVal::DoubleVal(v) => EncodedValue::Double(*v),
        EncodedVal::StringVal(s) => {
            let idx = dex.intern_string(s);
            EncodedValue::String(idx)
        }
        EncodedVal::TypeVal(desc) => {
            let idx = dex.intern_type(desc);
            EncodedValue::Type(idx)
        }
    }
}

fn annotation_to_dex(ann: &AnnotationItem, dex: &mut DexFile) -> DexAnnotationItem {
    let visibility = match ann.visibility {
        0 => DexAnnotationVisibility::Build,
        1 => DexAnnotationVisibility::Runtime,
        2 => DexAnnotationVisibility::System,
        _ => DexAnnotationVisibility::Build,
    };
    let type_idx = dex.intern_type(&ann.annotation_type);
    let elements = ann
        .elements
        .iter()
        .map(|el| {
            let name_idx = dex.intern_string(&el.name);
            let value = encoded_val_to_dex(&el.value, dex);
            DexAnnotationElement {
                name: name_idx,
                value,
            }
        })
        .collect();
    DexAnnotationItem {
        visibility,
        type_: type_idx,
        elements,
    }
}

fn convert_fingerprint(fp: &FingerprintDef) -> Fingerprint {
    Fingerprint {
        name: fp.name.clone(),
        defining_class: fp.defining_class.clone(),
        access_flags: fp.access_flags.map(AccessFlags::from_bits_truncate),
        return_type: fp.return_type.clone(),
        parameters: fp.parameters.clone(),
        opcodes: fp.opcodes.as_ref().map(|ops| {
            ops.iter()
                .map(|o| {
                    if *o < 0 {
                        InstructionPattern::Any
                    } else {
                        InstructionPattern::OpcodeValue(*o as u16)
                    }
                })
                .collect()
        }),
        strings: fp.strings.clone(),
    }
}

fn collect_fields(
    dex: &DexFile,
    class: &stitch_apk::stitch_dex::ClassDef,
    statics: bool,
    instances: bool,
) -> Vec<FieldInfo> {
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
                access_flags: f.access_flags.bits(),
            });
        }
    }
    fields
}

fn get_insn_register(m: u32, index: u32, reg_pos: usize) -> u16 {
    with_ctx(|ctx| {
        let mh = match with_handles(|h| h.get_method(m)) {
            Some(mh) => mh,
            None => return 0,
        };
        let dex = match ctx.dex_file(mh.dex_idx) {
            Some(d) => d,
            None => return 0,
        };
        let method = get_method_ref(dex, mh);
        method
            .code
            .as_ref()
            .and_then(|c| {
                c.instructions
                    .get(index as usize)
                    .and_then(|insn| insn.registers_used().get(reg_pos).copied())
            })
            .unwrap_or(0)
    })
}
