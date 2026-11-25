// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The AArch64 instructions that compute data addresses, and the registers the others overwrite.

/// An instruction's part in computing an address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Address {
    /// `adr` or `adrp`: `register` holds `value`.
    Load { register: u8, value: u64 },
    /// `add` of an immediate: `register` holds `base` plus `offset`.
    Add { register: u8, base: u8, offset: u64 },
    /// A load from `base` plus `offset`. What it writes is [`clobbers`].
    Read { base: u8, offset: u64 },
}

pub(super) fn decode(instruction: u32, pc: u64) -> Option<Address> {
    let register = (instruction & 0x1f) as u8;
    let base = ((instruction >> 5) & 0x1f) as u8;
    let immediate = u64::from((instruction >> 10) & 0xfff);
    if instruction & 0x1f00_0000 == 0x1000_0000 {
        let low = u64::from((instruction >> 29) & 0x3);
        let high = u64::from((instruction >> 5) & 0x7_ffff);
        let displacement = sign_extend((high << 2) | low, 21);
        let value = if instruction & 0x8000_0000 == 0 {
            pc.wrapping_add_signed(displacement)
        } else {
            (pc & !0xfff).wrapping_add_signed(displacement << 12)
        };
        return Some(Address::Load { register, value });
    }
    if instruction & 0xff80_0000 == 0x9100_0000 {
        let shift = if instruction & 0x0040_0000 == 0 {
            0
        } else {
            12
        };
        return Some(Address::Add {
            register,
            base,
            offset: immediate << shift,
        });
    }
    // Loads with an unsigned, size-scaled offset: the integer forms by their size field, and the
    // SIMD forms, whose 128-bit variant sets opc bit 1.
    let scale = match instruction & 0x3fc0_0000 {
        0x3940_0000 | 0x3980_0000 | 0x39c0_0000 | 0x3d40_0000 => Some(instruction >> 30),
        0x3dc0_0000 if instruction >> 30 == 0 => Some(4),
        _ => None,
    }?;
    Some(Address::Read {
        base,
        offset: immediate << scale,
    })
}

pub(super) fn sign_extend(value: u64, bits: u32) -> i64 {
    let shift = 64 - bits;
    ((value << shift) as i64) >> shift
}

/// What an instruction overwrites, so register tracking forgets the value it held. Covers the
/// instructions [`decode`] does not model as computing an address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Clobber {
    /// Writes no general register the tracker follows.
    None,
    /// Writes this general register an unknown value.
    Register(u8),
    /// A procedure call: writes every caller-saved register.
    CallerSaved,
}

/// What `instruction` overwrites, for every instruction except those [`decode`] models as
/// computing an address. Each instruction clears at most one register, so a load pair's second
/// register, CASP's second status register, base-register writeback, the source and size registers
/// of a memory copy or set, and moves out of the floating-point registers keep a stale value.
pub(super) fn clobbers(instruction: u32) -> Clobber {
    let register = (instruction & 0x1f) as u8;
    // BL, and the register-form calls BLR / BLRAA / BLRAB (opcode bits 23..21 = 0b001).
    if instruction & 0xfc00_0000 == 0x9400_0000
        || (instruction >> 25 == 0b110_1011 && (instruction >> 21) & 0x7 == 0b001)
    {
        return Clobber::CallerSaved;
    }
    // MRS reads a system register into Rt, such as the stack-guard load `mrs x, tpidr_el0`; the
    // rest of the system class writes no general register.
    if instruction & 0xfff0_0000 == 0xd530_0000 {
        return Clobber::Register(register);
    }
    match (instruction >> 25) & 0xf {
        // Data processing, immediate or register: writes Rd. adr/adrp and add-immediate are
        // decoded as addresses and never reach here.
        0b1000 | 0b1001 | 0b0101 | 0b1101 => Clobber::Register(register),
        0b0100 | 0b0110 | 0b1100 | 0b1110 => load_store(instruction),
        _ => Clobber::None,
    }
}

/// The general register a load or store writes, by its encoding group in bits 29..28. A store
/// writes none, except the status register of a store-exclusive. Bit 26 marks a transfer of SIMD
/// and floating-point registers, which writes no general register.
fn load_store(instruction: u32) -> Clobber {
    let bit = |index: u32| (instruction >> index) & 1 == 1;
    let transfer = Clobber::Register((instruction & 0x1f) as u8);
    let status = Clobber::Register(((instruction >> 16) & 0x1f) as u8);
    let low_opcode = (instruction >> 10) & 0x3;
    // L in the exclusive and pair groups; the low bit of opc in the single-register groups.
    let load = bit(22);
    // opc, bits 23..22, of the single-register forms: 0b00 stores, the rest load or prefetch.
    let opc_loads = (instruction >> 22) & 0x3 != 0;
    match (instruction >> 28) & 0x3 {
        0b00 if bit(26) => Clobber::None,
        // Compare-and-swap writes the old value to Rs: CAS has o2 (bit 23) and o1 (bit 21) set,
        // CASP has o1 set and bit 31 clear.
        0b00 if bit(21) && (bit(23) || !bit(31)) => status,
        0b00 if load => transfer,
        // A store-release (o2 set) writes nothing; a store-exclusive writes its status to Rs.
        0b00 if bit(23) => Clobber::None,
        0b00 => status,
        // Memory copy and set write their destination register, with or without bit 26.
        0b01 if bit(24) && !bit(21) && low_opcode == 0b01 => transfer,
        _ if bit(26) => Clobber::None,
        // Load literal: always a load or prefetch, with an offset where opc would be.
        0b01 if !bit(24) => transfer,
        0b10 if load => transfer,
        0b10 => Clobber::None,
        // Atomic memory operations and pointer-authenticated loads write Rt whatever opc holds.
        0b11 if !bit(24) && bit(21) && low_opcode != 0b10 => transfer,
        _ if opc_loads => transfer,
        _ => Clobber::None,
    }
}
