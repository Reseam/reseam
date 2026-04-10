use boltffi::export;

use crate::kotlin::convert::dex_to_kotlin;
use crate::kotlin::types::{
    FieldAccessSiteResult, Instruction, InstructionHit, MethodCallSiteResult, MethodRef,
};
use crate::kotlin::{get_method_ref, scan_location, with_ctx, with_handles};

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
        let method = match get_method_ref(dex, mh) {
            Some(m) => m,
            None => return Vec::new(),
        };
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
        let method = match get_method_ref(dex, mh) {
            Some(m) => m,
            None => return nop(),
        };
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
        let method = match get_method_ref(dex, mh) {
            Some(m) => m,
            None => return 0,
        };
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
        let method = get_method_ref(dex, mh)?;
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
        let method = get_method_ref(dex, mh)?;
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
        let method = get_method_ref(dex, mh)?;
        method.code.as_ref().and_then(|c| {
            c.instructions
                .iter()
                .position(|insn| insn.literal() == Some(literal))
                .map(|i| i as u32)
        })
    })
}

#[export]
pub fn index_of_first_literal_reversed(m: u32, literal: i64) -> Option<u32> {
    with_ctx(|ctx| {
        let mh = with_handles(|h| h.get_method(m))?;
        let dex = ctx.dex_file(mh.dex_idx)?;
        let method = get_method_ref(dex, mh)?;
        method.code.as_ref().and_then(|c| {
            c.instructions
                .iter()
                .enumerate()
                .rev()
                .find(|(_, insn)| insn.literal() == Some(literal))
                .map(|(i, _)| i as u32)
        })
    })
}

#[export]
pub fn contains_literal(m: u32, literal: i64) -> bool {
    index_of_first_literal(m, literal).is_some()
}

#[export]
pub fn index_of_first_string(m: u32, s: String) -> Option<u32> {
    with_ctx(|ctx| {
        let mh = with_handles(|h| h.get_method(m))?;
        let dex = ctx.dex_file(mh.dex_idx)?;
        let target_idx = dex.find_string_idx(&s)?;
        let method = get_method_ref(dex, mh)?;
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
        let method = match get_method_ref(dex, mh) {
            Some(m) => m,
            None => return Vec::new(),
        };
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
pub fn index_of_first_method_call(
    m: u32,
    defining_class: String,
    method_name: String,
    start: u32,
) -> Option<u32> {
    with_ctx(|ctx| {
        let mh = with_handles(|h| h.get_method(m))?;
        let dex = ctx.dex_file(mh.dex_idx)?;
        let method = get_method_ref(dex, mh)?;
        method.code.as_ref().and_then(|c| {
            c.instructions
                .iter()
                .enumerate()
                .skip(start as usize)
                .find(|(_, insn)| {
                    insn.method_ref().map_or(false, |mr| {
                        let mid = &dex.methods[mr.0 as usize];
                        dex.type_descriptor(mid.class) == defining_class
                            && dex.string(mid.name) == method_name
                    })
                })
                .map(|(i, _)| i as u32)
        })
    })
}

#[export]
pub fn index_of_first_field_access(
    m: u32,
    op: i32,
    field_type: Option<String>,
    defining_class: Option<String>,
    start: u32,
) -> Option<u32> {
    with_ctx(|ctx| {
        let mh = with_handles(|h| h.get_method(m))?;
        let dex = ctx.dex_file(mh.dex_idx)?;
        let method = get_method_ref(dex, mh)?;
        let target_op = if op >= 0 { Some(op as u16) } else { None };
        method.code.as_ref().and_then(|c| {
            c.instructions
                .iter()
                .enumerate()
                .skip(start as usize)
                .find(|(_, insn)| {
                    if let Some(top) = target_op {
                        if insn.opcode() != Some(top) {
                            return false;
                        }
                    }
                    insn.field_ref().map_or(false, |fr| {
                        let fid = &dex.fields[fr.0 as usize];
                        if let Some(ref ft) = field_type {
                            if dex.type_descriptor(fid.type_) != ft.as_str() {
                                return false;
                            }
                        }
                        if let Some(ref dc) = defining_class {
                            if dex.type_descriptor(fid.class) != dc.as_str() {
                                return false;
                            }
                        }
                        true
                    })
                })
                .map(|(i, _)| i as u32)
        })
    })
}

#[export]
pub fn index_of_opcode_sequence(m: u32, opcodes: Vec<i32>, start: u32) -> Option<u32> {
    with_ctx(|ctx| {
        let mh = with_handles(|h| h.get_method(m))?;
        let dex = ctx.dex_file(mh.dex_idx)?;
        let method = get_method_ref(dex, mh)?;
        method.code.as_ref().and_then(|c| {
            let insns = &c.instructions;
            if opcodes.is_empty() || insns.len() < opcodes.len() {
                return None;
            }
            let end = insns.len() - opcodes.len() + 1;
            for i in (start as usize)..end {
                let mut matched = true;
                for (j, &op) in opcodes.iter().enumerate() {
                    if op < 0 {
                        continue;
                    }
                    if insns[i + j].opcode() != Some(op as u16) {
                        matched = false;
                        break;
                    }
                }
                if matched {
                    return Some(i as u32);
                }
            }
            None
        })
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
pub fn find_method_call_sites(
    class_names: Vec<String>,
    method_names: Vec<String>,
) -> Vec<MethodCallSiteResult> {
    if class_names.len() != method_names.len() {
        return Vec::new();
    }
    let targets: Vec<(String, String)> = class_names.into_iter().zip(method_names).collect();
    let locations: Vec<(usize, usize, usize, bool, usize, usize)> = with_ctx(|ctx| {
        let hits = ctx.find_method_call_sites(&targets);
        hits.iter()
            .filter_map(|hit| {
                let (actual, is_virtual) =
                    scan_location(ctx, hit.loc.dex_idx, hit.loc.class_idx, hit.loc.method_idx)?;
                Some((
                    hit.loc.dex_idx,
                    hit.loc.class_idx,
                    actual,
                    is_virtual,
                    hit.loc.insn_idx,
                    hit.target_index,
                ))
            })
            .collect()
    });
    locations
        .into_iter()
        .map(|(di, ci, mi, iv, ii, ti)| MethodCallSiteResult {
            method: with_handles(|h| h.alloc_method(di, ci, mi, iv)),
            index: ii as u32,
            target_index: ti as u32,
        })
        .collect()
}

#[export]
pub fn find_field_access_sites(
    class_names: Vec<String>,
    field_names: Vec<String>,
) -> Vec<FieldAccessSiteResult> {
    if class_names.len() != field_names.len() {
        return Vec::new();
    }
    let targets: Vec<(String, String)> = class_names.into_iter().zip(field_names).collect();
    let locations: Vec<(usize, usize, usize, bool, usize, usize)> = with_ctx(|ctx| {
        let hits = ctx.find_field_access_sites(&targets);
        hits.iter()
            .filter_map(|hit| {
                let (actual, is_virtual) =
                    scan_location(ctx, hit.loc.dex_idx, hit.loc.class_idx, hit.loc.method_idx)?;
                Some((
                    hit.loc.dex_idx,
                    hit.loc.class_idx,
                    actual,
                    is_virtual,
                    hit.loc.insn_idx,
                    hit.target_index,
                ))
            })
            .collect()
    });
    locations
        .into_iter()
        .map(|(di, ci, mi, iv, ii, ti)| FieldAccessSiteResult {
            method: with_handles(|h| h.alloc_method(di, ci, mi, iv)),
            index: ii as u32,
            target_index: ti as u32,
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
pub fn all_method_handles() -> Vec<u32> {
    with_ctx(|ctx| {
        let mut handles = Vec::new();
        for dex_idx in 0..ctx.dex_count() {
            let dex = match ctx.dex_file(dex_idx) {
                Some(d) => d,
                None => continue,
            };
            for (class_idx, class) in dex.classes.iter().enumerate() {
                if let Some(data) = &class.class_data {
                    for (mi, _) in data.direct_methods.iter().enumerate() {
                        handles.push(with_handles(|h| {
                            h.alloc_method(dex_idx, class_idx, mi, false)
                        }));
                    }
                    for (mi, _) in data.virtual_methods.iter().enumerate() {
                        handles.push(with_handles(|h| {
                            h.alloc_method(dex_idx, class_idx, mi, true)
                        }));
                    }
                }
            }
        }
        handles
    })
}

#[export]
pub fn find_instructions_by_invoke(
    defining_class: String,
    method_name: String,
) -> Vec<InstructionHit> {
    find_method_call_sites(vec![defining_class], vec![method_name])
        .into_iter()
        .map(|site| InstructionHit {
            method: site.method,
            index: site.index,
        })
        .collect()
}

#[export]
pub fn instruction_string_ref(m: u32, index: u32) -> Option<String> {
    with_ctx(|ctx| {
        let mh = with_handles(|h| h.get_method(m))?;
        let dex = ctx.dex_file(mh.dex_idx)?;
        let method = get_method_ref(dex, mh)?;
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
        let method = get_method_ref(dex, mh)?;
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
pub fn instruction_field_ref(m: u32, index: u32) -> Option<crate::kotlin::types::FieldRef> {
    with_ctx(|ctx| {
        let mh = with_handles(|h| h.get_method(m))?;
        let dex = ctx.dex_file(mh.dex_idx)?;
        let method = get_method_ref(dex, mh)?;
        method.code.as_ref().and_then(|c| {
            c.instructions.get(index as usize).and_then(|insn| {
                insn.field_ref().map(|field_idx| {
                    let fid = &dex.fields[field_idx.0 as usize];
                    crate::kotlin::types::FieldRef {
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
        let method = get_method_ref(dex, mh)?;
        method.code.as_ref().and_then(|c| {
            c.instructions.get(index as usize).and_then(|insn| {
                insn.type_ref()
                    .map(|type_idx| dex.type_descriptor(type_idx).to_string())
            })
        })
    })
}

fn nop() -> Instruction {
    Instruction::Simple(crate::kotlin::types::SimpleInsn { opcode: 0x00 })
}
