use boltffi::export;
use stitch_apk::stitch_dex::find_contiguous_free_registers as dex_find_contiguous_free_registers;

use crate::kotlin::{get_method_mut, get_method_ref, with_ctx, with_handles};

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
        let method = match get_method_mut(dex, mh) {
            Some(m) => m,
            None => return,
        };
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
            .and_then(|m| m.code.as_ref())
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
            .and_then(|m| m.code.as_ref())
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
            .and_then(|m| m.code.as_ref())
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
        let method = match get_method_ref(dex, mh) {
            Some(m) => m,
            None => return 0,
        };
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
        let method = match get_method_ref(dex, mh) {
            Some(m) => m,
            None => return Vec::new(),
        };
        method
            .code
            .as_ref()
            .and_then(|c| ctx.find_free_registers(c, at_index as usize, count as usize, &exclude))
            .unwrap_or_default()
    })
}

#[export]
pub fn find_contiguous_free_registers(
    m: u32,
    at_index: u32,
    count: u32,
    exclude: Vec<u16>,
) -> Vec<u16> {
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
        method
            .code
            .as_ref()
            .and_then(|c| dex_find_contiguous_free_registers(c, at_index as usize, count as usize, &exclude))
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
        let method = match get_method_ref(dex, mh) {
            Some(m) => m,
            None => return 0,
        };
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
        let method = match get_method_ref(dex, mh) {
            Some(m) => m,
            None => return 0,
        };
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
