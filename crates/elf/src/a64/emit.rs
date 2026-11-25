// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The AArch64 code a constant return writes.

const PACIASP: u32 = 0xd503_233f;
const PACIBSP: u32 = 0xd503_237f;
const AUTIASP: u32 = 0xd503_23bf;
const AUTIBSP: u32 = 0xd503_23ff;
const RET: u32 = 0xd65f_03c0;

/// The instruction a function built with branch protection starts with, where indirect branches
/// may land.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LandingPad {
    /// `bti`, which only marks the landing.
    Bti,
    /// `paciasp` or `pacibsp`, which also sign the return address. The function must check it
    /// with this authentication before it returns, or the signed address faults on CPUs with
    /// pointer authentication.
    Sign { authenticate: u32 },
}

fn landing_pad(instruction: u32) -> Option<LandingPad> {
    match instruction {
        PACIASP => Some(LandingPad::Sign {
            authenticate: AUTIASP,
        }),
        PACIBSP => Some(LandingPad::Sign {
            authenticate: AUTIBSP,
        }),
        _ if instruction & 0xffff_ff3f == 0xd503_241f => Some(LandingPad::Bti),
        _ => None,
    }
}

/// The code that makes a function starting with `first` return `value` in `w0`, and its offset in
/// the function. A leading landing pad of branch protection stays, so indirect calls into the
/// function remain valid, and a return address it signed is authenticated before the return.
pub(crate) fn return_code(first: u32, value: i32) -> (u64, Vec<u8>) {
    let pad = landing_pad(first);
    let code = return_constant(value, pad)
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    (pad.map_or(0, |_| 4), code)
}

/// The code after `pad` that sets `w0` to `value` and returns, as the fewest instructions.
fn return_constant(value: i32, pad: Option<LandingPad>) -> Vec<u32> {
    let bits = value as u32;
    let low = bits & 0xffff;
    let high = bits >> 16;
    let mut code = if high == 0 {
        vec![0x5280_0000 | (low << 5)]
    } else if high == 0xffff {
        vec![0x1280_0000 | ((!bits & 0xffff) << 5)]
    } else if low == 0 {
        vec![0x52a0_0000 | (high << 5)]
    } else {
        vec![0x5280_0000 | (low << 5), 0x72a0_0000 | (high << 5)]
    };
    if let Some(LandingPad::Sign { authenticate }) = pad {
        code.push(authenticate);
    }
    code.push(RET);
    code
}
