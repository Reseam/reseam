// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

/// Guards the `Instruction` size (was 40 B). 24 is the floor without
/// double-boxing the rare `RawInstruction`; the common variants pack into it.
#[test]
fn instruction_stays_compact() {
    let size = std::mem::size_of::<reseam_dex::Instruction>();
    assert!(size <= 24, "Instruction grew to {size} bytes (was 24)");
}
