// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

use crate::error::{Result, invalid};
use crate::instruction::Instruction;
use crate::instruction::branch::Addresses;
use crate::model::code::CodeItem;

/// What a label marks. The order is the order labels at one place print in,
/// try ends first so a `.catch` follows the range it closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum LabelKind {
    TryEnd,
    TryStart,
    Catch,
    CatchAll,
    Cond,
    Goto,
    PackedSwitch,
    SparseSwitch,
    PackedSwitchData,
    SparseSwitchData,
    Array,
}

impl LabelKind {
    /// The label of the payload `instruction` reads, and of the cases its
    /// payload lists when it is a switch.
    pub(super) fn payload(instruction: &Instruction) -> (Self, Option<Self>) {
        match instruction {
            Instruction::PackedSwitch { .. } => (Self::PackedSwitchData, Some(Self::PackedSwitch)),
            Instruction::SparseSwitch { .. } => (Self::SparseSwitchData, Some(Self::SparseSwitch)),
            _ => (Self::Array, None),
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Self::TryEnd => "try_end",
            Self::TryStart => "try_start",
            Self::Catch => "catch",
            Self::CatchAll => "catchall",
            Self::Cond => "cond",
            Self::Goto => "goto",
            Self::PackedSwitch => "pswitch",
            Self::SparseSwitch => "sswitch",
            Self::PackedSwitchData => "pswitch_data",
            Self::SparseSwitchData => "sswitch_data",
            Self::Array => "array",
        }
    }
}

/// A label as baksmali names it with sequential labels: its kind and its
/// number among labels of that kind in address order, written in hex.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Label {
    pub(super) kind: LabelKind,
    number: u32,
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, ":{}_{:x}", self.kind.prefix(), self.number)
    }
}

/// The labels of one body by the instruction index they precede; the body's
/// length stands for its end.
pub(super) struct Labels {
    at: BTreeMap<usize, Vec<Label>>,
    /// The switch that reads each switch payload, by payload index.
    switch_of: HashMap<usize, usize>,
    addresses: Addresses,
}

impl Labels {
    pub(super) fn of(code: &CodeItem) -> Result<Self> {
        let instructions = code.instructions();
        let addresses = Addresses::of(instructions);
        let target = |source: usize, delta: i32| {
            addresses.target(source, delta).ok_or_else(|| {
                invalid(
                    "code item",
                    format!("instruction {source} points {delta} code units outside the body"),
                )
            })
        };
        let at_address = |address: u32| {
            addresses.index_of(address).ok_or_else(|| {
                invalid(
                    "code item",
                    format!("address {address} is not an instruction boundary"),
                )
            })
        };
        let mut marks = BTreeSet::new();
        let mut switch_of = HashMap::new();
        for (index, instruction) in instructions.iter().enumerate() {
            if let Some(delta) = instruction.branch_offset() {
                let kind = if instruction.is_condition() {
                    LabelKind::Cond
                } else {
                    LabelKind::Goto
                };
                marks.insert((target(index, delta)?, kind));
            }
            let Some(delta) = instruction.payload_offset() else {
                continue;
            };
            let payload = target(index, delta)?;
            let (data, case) = LabelKind::payload(instruction);
            marks.insert((payload, data));
            if let Some(case) = case {
                switch_of.insert(payload, index);
                for delta in instructions[payload].switch_targets().unwrap_or_default() {
                    marks.insert((target(index, delta)?, case));
                }
            }
        }
        for item in code.tries() {
            let end = item
                .start_addr
                .checked_add(u32::from(item.insn_count))
                .ok_or_else(|| invalid("code item", "try range overflows"))?;
            marks.insert((at_address(item.start_addr)?, LabelKind::TryStart));
            marks.insert((at_address(end)?, LabelKind::TryEnd));
        }
        for handler in code.catch_handlers() {
            for catch in &handler.typed_catches {
                marks.insert((at_address(catch.addr)?, LabelKind::Catch));
            }
            if let Some(address) = handler.catch_all_addr {
                marks.insert((at_address(address)?, LabelKind::CatchAll));
            }
        }
        let mut counts: HashMap<LabelKind, u32> = HashMap::new();
        let mut at: BTreeMap<usize, Vec<Label>> = BTreeMap::new();
        for (index, kind) in marks {
            let count = counts.entry(kind).or_default();
            at.entry(index).or_default().push(Label {
                kind,
                number: *count,
            });
            *count += 1;
        }
        Ok(Self {
            at,
            switch_of,
            addresses,
        })
    }

    /// The labels before instruction `index`, in print order.
    pub(super) fn at(&self, index: usize) -> &[Label] {
        self.at.get(&index).map_or(&[], Vec::as_slice)
    }

    /// The label of `kind` that instruction `source` reaches `delta` code units away.
    pub(super) fn reached(&self, source: usize, delta: i32, kind: LabelKind) -> Label {
        self.find(
            self.addresses
                .target(source, delta)
                .expect("targets were checked when the labels were made"),
            kind,
        )
    }

    /// The label of `kind` at the instruction starting at `address`.
    pub(super) fn at_address(&self, address: u32, kind: LabelKind) -> Label {
        self.find(
            self.index_of(address)
                .expect("addresses were checked when the labels were made"),
            kind,
        )
    }

    /// The index of the instruction starting at `address`, or of the end.
    pub(super) fn index_of(&self, address: u32) -> Option<usize> {
        self.addresses.index_of(address)
    }

    /// The switch whose payload is instruction `payload`.
    pub(super) fn switch_of(&self, payload: usize) -> Option<usize> {
        self.switch_of.get(&payload).copied()
    }

    fn find(&self, index: usize, kind: LabelKind) -> Label {
        *self
            .at(index)
            .iter()
            .find(|label| label.kind == kind)
            .expect("every referenced place was labelled")
    }
}
