// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_apk::ApkFile;
use reseam_dex::{DexFile, ParseOptions};

// Distinct incoming words expose lost arguments and overlapping wide moves.
pub fn hook_calls(code: &reseam_dex::CodeItem, incoming: &[i64]) -> (Vec<Vec<i64>>, Option<i64>) {
    use reseam_dex::Instruction::*;
    let mut registers = vec![-1; usize::from(code.registers_size())];
    registers[usize::from(code.registers_size() - code.ins_size())..].copy_from_slice(incoming);
    let mut calls = Vec::new();
    let offsets: Vec<u32> = code
        .instructions()
        .iter()
        .scan(0, |addr, insn| {
            let current = *addr;
            *addr += insn.code_units();
            Some(current)
        })
        .collect();
    let target = |pc: usize, displacement: i32| {
        let address = offsets[pc].checked_add_signed(displacement).unwrap();
        offsets
            .iter()
            .position(|offset| *offset == address)
            .unwrap()
    };
    let mut pc = 0;
    for _ in 0..code.instructions().len() * 2 {
        let insn = &code.instructions()[pc];
        let movement = fixture_movement(insn);
        let wide_constant = match insn {
            ConstWide16 { dest, .. }
            | ConstWide32 { dest, .. }
            | ConstWide { dest, .. }
            | ConstWideHigh16 { dest, .. } => Some((*dest, insn.literal().unwrap())),
            _ => None,
        };
        if let Some((dest, value)) = wide_constant {
            registers[usize::from(dest)] = i64::from(value as u32);
            registers[usize::from(dest) + 1] = i64::from((value >> 32) as u32);
        } else if let Some(movement) = movement {
            registers.copy_within(
                movement.source..movement.source + movement.words,
                movement.destination,
            );
        } else if let Some(arguments) = fixture_invoke_arguments(insn) {
            calls.push(
                arguments
                    .into_iter()
                    .map(|register| registers[register])
                    .collect(),
            );
        } else if let Some((destination, value)) = fixture_scalar_write(insn, &registers) {
            registers[destination] = value;
        } else {
            match insn {
                IfEq { a, b, offset } | IfNe { a, b, offset }
                    if (registers[usize::from(*a)] == registers[usize::from(*b)])
                        == matches!(insn, IfEq { .. }) =>
                {
                    pc = target(pc, i32::from(*offset));
                    continue;
                }
                IfEqz { a, offset } | IfNez { a, offset }
                    if (registers[usize::from(*a)] == 0) == matches!(insn, IfEqz { .. }) =>
                {
                    pc = target(pc, i32::from(*offset));
                    continue;
                }
                Goto { offset } => {
                    pc = target(pc, i32::from(*offset));
                    continue;
                }
                Goto16 { offset } => {
                    pc = target(pc, i32::from(*offset));
                    continue;
                }
                Goto32 { offset } => {
                    pc = target(pc, *offset);
                    continue;
                }
                Nop | IfEqz { .. } | IfNez { .. } | IfEq { .. } | IfNe { .. } => {}
                Return { src } | ReturnObject { src } => {
                    return (calls, Some(registers[usize::from(*src)]));
                }
                ReturnWide { src } => {
                    return (
                        calls,
                        Some(
                            registers[usize::from(*src)] | (registers[usize::from(*src) + 1] << 32),
                        ),
                    );
                }
                ReturnVoid => return (calls, None),
                _ => panic!("unexpected fixture instruction: {insn:?}"),
            }
        }
        pc += 1;
    }
    panic!("fixture did not return");
}

pub fn fixture_scalar_write(
    instruction: &reseam_dex::Instruction,
    registers: &[i64],
) -> Option<(usize, i64)> {
    use reseam_dex::Instruction::*;
    use std::cmp::Ordering;
    let wide = |register: u8| {
        registers[usize::from(register)] as u64
            | ((registers[usize::from(register) + 1] as u64) << 32)
    };
    let compare = |order: Option<Ordering>| match order {
        Some(Ordering::Equal) => 0,
        Some(Ordering::Greater) => 1,
        None if matches!(instruction, CmpGFloat { .. } | CmpGDouble { .. }) => 1,
        Some(Ordering::Less) | None => -1,
    };
    let (dest, value) = match *instruction {
        Const4 { dest, value } => (dest, i64::from(value)),
        Const16 { dest, value } => (dest, i64::from(value)),
        Const { dest, value } => (dest, i64::from(value)),
        ConstHigh16 { dest, value } => (dest, i64::from(value) << 16),
        // Fixture references are either null or instances of the queried type.
        InstanceOf { dest, ref_, .. } => (dest, i64::from(registers[usize::from(ref_)] != 0)),
        CmpLong { dest, a, b } => (dest, compare(Some((wide(a) as i64).cmp(&(wide(b) as i64))))),
        CmpLFloat { dest, a, b } | CmpGFloat { dest, a, b } => (
            dest,
            compare(
                f32::from_bits(registers[usize::from(a)] as u32)
                    .partial_cmp(&f32::from_bits(registers[usize::from(b)] as u32)),
            ),
        ),
        CmpLDouble { dest, a, b } | CmpGDouble { dest, a, b } => (
            dest,
            compare(f64::from_bits(wide(a)).partial_cmp(&f64::from_bits(wide(b)))),
        ),
        _ => return None,
    };
    Some((usize::from(dest), value))
}

pub fn fixture_invoke_arguments(instruction: &reseam_dex::Instruction) -> Option<Vec<usize>> {
    use reseam_dex::Instruction::*;
    match instruction {
        InvokeStatic { args, .. }
        | InvokeVirtual { args, .. }
        | InvokeDirect { args, .. }
        | InvokeSuper { args, .. }
        | InvokeInterface { args, .. } => Some(args.iter().map(|r| usize::from(*r)).collect()),
        InvokeStaticRange {
            first_reg, count, ..
        }
        | InvokeVirtualRange {
            first_reg, count, ..
        }
        | InvokeDirectRange {
            first_reg, count, ..
        }
        | InvokeSuperRange {
            first_reg, count, ..
        }
        | InvokeInterfaceRange {
            first_reg, count, ..
        } => {
            Some((usize::from(*first_reg)..usize::from(*first_reg) + usize::from(*count)).collect())
        }
        _ => None,
    }
}

pub struct FixtureMovement {
    destination: usize,
    source: usize,
    words: usize,
}

pub fn fixture_movement(instruction: &reseam_dex::Instruction) -> Option<FixtureMovement> {
    use reseam_dex::Instruction::*;
    let (destination, source) = match *instruction {
        Move { dest, src } | MoveObject { dest, src } | MoveWide { dest, src } => {
            (usize::from(dest), usize::from(src))
        }
        MoveFrom16 { dest, src }
        | MoveObjectFrom16 { dest, src }
        | MoveWideFrom16 { dest, src } => (usize::from(dest), usize::from(src)),
        Move16 { dest, src } | MoveObject16 { dest, src } | MoveWide16 { dest, src } => {
            (usize::from(dest), usize::from(src))
        }
        _ => return None,
    };
    let words = if matches!(
        instruction,
        MoveWide { .. } | MoveWideFrom16 { .. } | MoveWide16 { .. }
    ) {
        2
    } else {
        1
    };
    Some(FixtureMovement {
        destination,
        source,
        words,
    })
}

pub fn invoked_by(dex: &DexFile, owner: &str, name: &str) -> Vec<String> {
    let method = read_method(dex, owner, name).unwrap();
    method
        .code
        .as_ref()
        .unwrap()
        .instructions()
        .iter()
        .filter_map(reseam_dex::Instruction::method_ref)
        .map(|idx| {
            let id = dex.method_id(idx);
            format!(
                "{}->{}",
                dex.type_descriptor(id.class),
                dex.strings().get(id.name)
            )
        })
        .collect()
}

pub fn dex_bytes(dex: &DexFile) -> Vec<u8> {
    let (spooled, _) = reseam_dex::write_spooled(dex, None).unwrap();
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut spooled.reader(), &mut bytes).unwrap();
    bytes
}

pub fn written_dex(apk: &ApkFile, index: usize) -> DexFile {
    reseam_dex::parse(
        &dex_bytes(apk.dex().get(index).unwrap()),
        ParseOptions::default(),
    )
    .unwrap()
}

pub fn patched_code(apk: &ApkFile, owner: &str, name: &str) -> reseam_dex::CodeItem {
    read_method(&written_dex(apk, 0), owner, name)
        .unwrap()
        .code
        .map(std::sync::Arc::unwrap_or_clone)
        .unwrap()
}

pub fn read_method(dex: &DexFile, owner: &str, name: &str) -> Option<reseam_dex::EncodedMethod> {
    let class = dex.find_class_index(owner)?;
    let name = dex.strings().find(name)?;
    let hit = dex
        .scan_methods_find(&reseam_dex::RefQuery::default(), |view| {
            Ok(
                (view.class_idx == class && dex.method_id(view.method).name == name)
                    .then(|| view.hit()),
            )
        })
        .unwrap()?;
    dex.decode_method_at(hit.class_idx, hit.method_pos, hit.kind)
        .unwrap()
}
