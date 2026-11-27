// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs;
use std::path::Path;

use anyhow::Result;

pub fn write(directory: &Path) -> Result<()> {
    let opcodes = catalogue();
    let entries = opcodes
        .iter()
        .map(|op| format!("    {}(0x{:02x})", kotlin_opcode_name(op.name), op.value))
        .collect::<Vec<_>>()
        .join(",\n");
    fs::create_dir_all(directory)?;
    fs::write(directory.join("OpcodeKinds.kt"), opcode_kinds(&opcodes))?;
    fs::write(directory.join("Opcode.kt"), opcode_enum(&entries))?;
    Ok(())
}

struct Opcode {
    name: &'static str,
    value: u16,
    invokes: bool,
    returns: bool,
    registers: Vec<(&'static str, &'static str)>,
}

fn catalogue() -> Vec<Opcode> {
    macro_rules! operand {
        (RegList) => {
            reseam_dex::RegList::default()
        };
        ($index:ident) => {
            Default::default()
        };
    }
    macro_rules! field {
        (StringIdx) => {
            reseam_dex::StringIdx(0)
        };
        (TypeIdx) => {
            reseam_dex::TypeIdx(0)
        };
        (ProtoIdx) => {
            reseam_dex::ProtoIdx(0)
        };
        (FieldIdx) => {
            reseam_dex::FieldIdx(0)
        };
        (MethodIdx) => {
            reseam_dex::MethodIdx(0)
        };
        (CallSiteIdx) => {
            reseam_dex::CallSiteIdx(0)
        };
        (MethodHandleIdx) => {
            reseam_dex::MethodHandleIdx(0)
        };
        ($other:ident) => {
            operand!($other)
        };
    }
    macro_rules! instruction {
        ($variant:ident, []) => { reseam_dex::Instruction::$variant };
        ($variant:ident, [{$($name:ident:$type:ident,)*}]) => { reseam_dex::Instruction::$variant { $($name:field!($type),)* } };
    }
    macro_rules! fixed {
        ($variant:ident, $definition:tt, $name:ident, $units:literal, [$(($kind:ident, $access:ident)),*], $result:ident) => {{
            let instruction = instruction!($variant, $definition);
            $result.push(Opcode {
                name: stringify!($name),
                value: instruction
                    .opcode()
                    .expect("fixed instructions have opcodes"),
                invokes: instruction.is_invoke(),
                returns: instruction.is_return(),
                registers: vec![$((stringify!($kind), stringify!($access))),*],
            });
        }};
        ($variant:ident, $definition:tt, $name:ident, $variable:tt, $registers:tt, $result:ident) => {};
    }
    macro_rules! rows {
        ($($variant:ident [$($shape:tt)*] [$($definition:tt)*] => $name:ident $opcode:expr, $units:tt; [$($register:ident:$rt:ident $kind:ident $access:ident ($max:expr)),*]; $args:ident; [$($index:ident:$it:ident $pool:ident $at:literal $width:ident),*]; $format:ident($($slot:ident),*);)*) => {{
            let mut result = Vec::new();
            $(fixed!($variant, [$($definition)*], $name, $units, [$(($kind, $access)),*], result);)*
            result
        }};
    }
    reseam_dex::instruction_catalogue!(rows)
}

fn opcode_kinds(opcodes: &[Opcode]) -> String {
    let values = |keep: fn(&Opcode) -> bool| {
        opcodes
            .iter()
            .filter(|op| keep(op))
            .map(|op| op.value.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let invokes = values(|op| op.invokes);
    let returns = values(|op| op.returns);
    let registers = opcodes
        .iter()
        .filter(|op| !op.registers.is_empty())
        .map(|op| {
            let uses = op
                .registers
                .iter()
                .map(|&(kind, access)| register_use(kind, access))
                .collect::<Vec<_>>()
                .join(", ");
            format!("        0x{:02x} to listOf({uses}),", op.value)
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "// Generated from the DEX instruction catalogue. Do not edit.\npackage app.reseam.patch.dex\n\ninternal object OpcodeKinds {{\n    fun invokes(opcode: Int): Boolean = when (opcode) {{ {invokes} -> true; else -> false }}\n    fun returns(opcode: Int): Boolean = when (opcode) {{ {returns} -> true; else -> false }}\n\n    val registers: Map<Int, List<RegisterUse>> = mapOf(\n{registers}\n    )\n}}\n"
    )
}

fn opcode_enum(entries: &str) -> String {
    format!(
        r"// Generated from the DEX instruction catalogue. Do not edit.
package app.reseam.patch.dex

/** Android DEX opcode values. */
enum class Opcode(val value: Int) {{
{entries};

    val isInvoke: Boolean get() = OpcodeKinds.invokes(value)
    val isReturn: Boolean get() = OpcodeKinds.returns(value)
    /** Execution never falls through to the next instruction. */
    val endsFlow: Boolean get() = isReturn || this == THROW || this == GOTO || this == GOTO_16 || this == GOTO_32
    val isMoveResult: Boolean get() = this == MOVE_RESULT || this == MOVE_RESULT_WIDE || this == MOVE_RESULT_OBJECT
    /** The register operands in encoding order, A first, with their words and access. */
    internal val registers: List<RegisterUse> get() = OpcodeKinds.registers[value].orEmpty()

    companion object {{
        private val byValue = entries.associateBy {{ it.value }}
        fun of(value: Int): Opcode? = byValue[value]
    }}
}}
"
    )
}

fn register_use(kind: &str, access: &str) -> String {
    let words = if kind == "Wide" { 2 } else { 1 };
    let access = match access {
        "Read" => "READ",
        "Write" => "WRITE",
        "ReadWrite" => "READ_WRITE",
        "Refine" => "REFINE",
        other => unreachable!(
            "the catalogue's accesses are Read, Write, ReadWrite and Refine, not {other}"
        ),
    };
    format!("RegisterUse({words}, RegisterAccess.{access})")
}

fn kotlin_opcode_name(name: &str) -> String {
    match name {
        "MOVE16" => "MOVE_16".into(),
        "MOVE_WIDE16" => "MOVE_WIDE_16".into(),
        "MOVE_OBJECT16" => "MOVE_OBJECT_16".into(),
        "CONST4" => "CONST_4".into(),
        "CONST16" => "CONST_16".into(),
        "CONST_WIDE16" => "CONST_WIDE_16".into(),
        "CONST_WIDE32" => "CONST_WIDE_32".into(),
        "GOTO16" => "GOTO_16".into(),
        "GOTO32" => "GOTO_32".into(),
        "RSUB_INT_LIT16" => "RSUB_INT".into(),
        _ => name
            .replace("2_ADDR", "_2ADDR")
            .replace("CMP_L_", "CMPL_")
            .replace("CMP_G_", "CMPG_"),
    }
}
