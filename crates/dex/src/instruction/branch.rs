// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::Instruction;
use crate::error::{Result, invalid};

macro_rules! conditions {
    ($($condition:ident <=> $inverse:ident { $($operand:ident),* }),* $(,)?) => {
        impl Instruction {
            /// The displacement of a goto or if-test, in code units from this
            /// instruction; payload references are not branches.
            pub fn branch_offset(&self) -> Option<i32> {
                match self {
                    Self::Goto { offset } => Some(i32::from(*offset)),
                    Self::Goto16 { offset } => Some(i32::from(*offset)),
                    Self::Goto32 { offset } => Some(*offset),
                    $(Self::$condition { offset, .. } | Self::$inverse { offset, .. } => Some(i32::from(*offset)),)*
                    _ => None,
                }
            }

            /// Points the branch `delta` code units away, failing when its
            /// encoding cannot hold the displacement.
            pub(crate) fn set_branch_offset(&mut self, delta: i32) -> Result<()> {
                let fits = |_| invalid("instruction rewrite", "branch displacement overflows its encoding");
                match self {
                    Self::Goto { offset } => *offset = i8::try_from(delta).map_err(fits)?,
                    Self::Goto32 { offset } => *offset = delta,
                    Self::Goto16 { offset }
                    $(| Self::$condition { offset, .. } | Self::$inverse { offset, .. })* => {
                        *offset = i16::try_from(delta).map_err(fits)?;
                    }
                    _ => return Err(invalid("instruction rewrite", "expected branch")),
                }
                Ok(())
            }

            pub(crate) fn is_condition(&self) -> bool {
                matches!(self, $(Self::$condition { .. } | Self::$inverse { .. })|*)
            }

            /// The same test with the opposite outcome, branching to the same place.
            pub(crate) fn inverted_condition(&self) -> Result<Self> {
                Ok(match *self {
                    $(Self::$condition { $($operand,)* offset } => Self::$inverse { $($operand,)* offset },
                    Self::$inverse { $($operand,)* offset } => Self::$condition { $($operand,)* offset },)*
                    _ => return Err(invalid("instruction rewrite", "expected condition")),
                })
            }
        }
    };
}

conditions! {
    IfEq <=> IfNe { a, b },
    IfLt <=> IfGe { a, b },
    IfGt <=> IfLe { a, b },
    IfEqz <=> IfNez { a },
    IfLtz <=> IfGez { a },
    IfGtz <=> IfLez { a },
}

impl Instruction {
    /// Whether execution can continue with the next instruction.
    pub(crate) fn falls_through(&self) -> bool {
        !matches!(
            self,
            Self::ReturnVoid
                | Self::Return { .. }
                | Self::ReturnWide { .. }
                | Self::ReturnObject { .. }
                | Self::Throw { .. }
                | Self::Goto { .. }
                | Self::Goto16 { .. }
                | Self::Goto32 { .. }
        ) && !self.is_payload()
    }

    pub(crate) fn is_payload(&self) -> bool {
        matches!(
            self,
            Self::PackedSwitchPayload(_)
                | Self::SparseSwitchPayload(_)
                | Self::FillArrayDataPayload(_)
        )
    }

    pub(crate) fn is_switch(&self) -> bool {
        matches!(self, Self::PackedSwitch { .. } | Self::SparseSwitch { .. })
    }

    /// The displacement of the payload a switch or fill-array-data reads.
    pub(crate) fn payload_offset_mut(&mut self) -> Option<&mut i32> {
        match self {
            Self::PackedSwitch { payload_offset, .. }
            | Self::SparseSwitch { payload_offset, .. }
            | Self::FillArrayData { payload_offset, .. } => Some(payload_offset),
            _ => None,
        }
    }

    pub(crate) fn payload_offset(&self) -> Option<i32> {
        match *self {
            Self::PackedSwitch { payload_offset, .. }
            | Self::SparseSwitch { payload_offset, .. }
            | Self::FillArrayData { payload_offset, .. } => Some(payload_offset),
            _ => None,
        }
    }

    /// The branch displacements a switch payload lists, relative to its switch.
    pub(crate) fn switch_targets(&self) -> Option<Vec<i32>> {
        match self {
            Self::PackedSwitchPayload(data) => Some(data.targets.clone()),
            Self::SparseSwitchPayload(data) => Some(
                data.keys_and_targets
                    .iter()
                    .map(|&(_, target)| target)
                    .collect(),
            ),
            _ => None,
        }
    }

    pub(crate) fn set_switch_targets(&mut self, targets: Vec<i32>) {
        match self {
            Self::PackedSwitchPayload(data) => data.targets = targets,
            Self::SparseSwitchPayload(data) => {
                for ((_, target), relocated) in data.keys_and_targets.iter_mut().zip(targets) {
                    *target = relocated;
                }
            }
            _ => {}
        }
    }
}

/// The code-unit address of each instruction of a body, then of its end.
#[derive(Debug, Clone)]
pub(crate) struct Addresses(Vec<u32>);

impl Addresses {
    pub(crate) fn of(instructions: &[Instruction]) -> Self {
        let mut addresses = Vec::with_capacity(instructions.len() + 1);
        let mut address = 0;
        for instruction in instructions {
            addresses.push(address);
            address += instruction.code_units();
        }
        addresses.push(address);
        Self(addresses)
    }

    /// The index of the instruction starting at `address`, or of the end.
    pub(crate) fn index_of(&self, address: u32) -> Option<usize> {
        self.0.binary_search(&address).ok()
    }

    /// The instruction `delta` code units from instruction `source`.
    pub(crate) fn target(&self, source: usize, delta: i32) -> Option<usize> {
        let address = i64::from(self.0[source]) + i64::from(delta);
        self.index_of(u32::try_from(address).ok()?)
    }
}

impl std::ops::Deref for Addresses {
    type Target = [u32];

    fn deref(&self) -> &[u32] {
        &self.0
    }
}
