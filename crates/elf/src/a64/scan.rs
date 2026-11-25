// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::decode::{Address, Clobber, clobbers, decode};
use super::unwind::FunctionTable;
use crate::data::LoadedData;
use crate::error::Result;
use crate::file::ElfImage;

/// The registers a procedure call may overwrite, per AAPCS64: the argument and temporary registers
/// x0-x18 and the link register x30. x19-x28 and the frame pointer are preserved across a call.
const CALLER_SAVED: [usize; 20] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 30,
];

/// Each instruction that computes an address in `data`, with that address, in code order.
///
/// Registers are followed from `adr` and `adrp` through immediate adds and loads within a
/// function, from its start in `functions` to the next one's.
pub(super) fn data_references(
    image: &ElfImage,
    functions: &FunctionTable,
    data: &LoadedData,
) -> Result<Vec<(u64, u64)>> {
    let mut references = Vec::new();
    for section in image.sections().iter().filter(|section| section.is_code()) {
        let code = image.contents(section)?;
        let mut starts = functions
            .starts(image)
            .skip_while(|&start| start < section.address)
            .peekable();
        let mut registers = [None::<u64>; 32];
        for (index, word) in code.as_chunks::<4>().0.iter().enumerate() {
            let pc = section.address.wrapping_add(index as u64 * 4);
            while starts.next_if(|&start| start <= pc).is_some() {
                registers = [None; 32];
            }
            let instruction = u32::from_le_bytes(*word);
            let (register, value) = match decode(instruction, pc) {
                Some(Address::Load { register, value }) => (register, Some(value)),
                Some(Address::Add {
                    register,
                    base,
                    offset,
                }) => (
                    register,
                    registers[usize::from(base)].map(|base| base.wrapping_add(offset)),
                ),
                read_or_other => {
                    if let Some(Address::Read { base, offset }) = read_or_other
                        && let Some(target) = registers[usize::from(base)]
                            .map(|base| base.wrapping_add(offset))
                            .filter(|&target| data.contains(target))
                    {
                        references.push((pc, target));
                    }
                    match clobbers(instruction) {
                        Clobber::None => {}
                        Clobber::Register(register) => registers[usize::from(register)] = None,
                        Clobber::CallerSaved => {
                            for register in CALLER_SAVED {
                                registers[register] = None;
                            }
                        }
                    }
                    continue;
                }
            };
            if let Some(value) = value
                && data.contains(value)
            {
                references.push((pc, value));
            }
            registers[usize::from(register)] = value;
        }
    }
    Ok(references)
}
