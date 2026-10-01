// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

pub(super) fn opcode_metadata() -> (String, String) {
    struct Opcode {
        name: &'static str,
        value: u16,
        units: u32,
        invokes: bool,
        returns: bool,
    }
    macro_rules! operand {
        (RegList) => {
            reseam_apk::reseam_dex::RegList::default()
        };
        ($index:ident) => {
            Default::default()
        };
    }
    macro_rules! field {
        (StringIdx) => {
            reseam_apk::reseam_dex::StringIdx(0)
        };
        (TypeIdx) => {
            reseam_apk::reseam_dex::TypeIdx(0)
        };
        (ProtoIdx) => {
            reseam_apk::reseam_dex::ProtoIdx(0)
        };
        (FieldIdx) => {
            reseam_apk::reseam_dex::FieldIdx(0)
        };
        (MethodIdx) => {
            reseam_apk::reseam_dex::MethodIdx(0)
        };
        (CallSiteIdx) => {
            reseam_apk::reseam_dex::CallSiteIdx(0)
        };
        (MethodHandleIdx) => {
            reseam_apk::reseam_dex::MethodHandleIdx(0)
        };
        ($other:ident) => {
            operand!($other)
        };
    }
    macro_rules! instruction {
        ($variant:ident, []) => { reseam_apk::reseam_dex::Instruction::$variant };
        ($variant:ident, [{$($name:ident:$type:ident,)*}]) => { reseam_apk::reseam_dex::Instruction::$variant { $($name:field!($type),)* } };
    }
    macro_rules! fixed {
        ($variant:ident, $definition:tt, $name:ident, $units:literal, $result:ident) => {{
            let instruction = instruction!($variant, $definition);
            $result.push(Opcode {
                name: stringify!($name),
                value: instruction
                    .opcode()
                    .expect("fixed instructions have opcodes"),
                units: instruction.code_units(),
                invokes: instruction.is_invoke(),
                returns: instruction.is_return(),
            });
        }};
        ($variant:ident, $definition:tt, $name:ident, $variable:tt, $result:ident) => {};
    }
    macro_rules! rows {
        ($($variant:ident [$($shape:tt)*] [$($definition:tt)*] => $name:ident $opcode:expr, $units:tt; [$($register:ident:$rt:ident $kind:ident $access:ident ($max:expr)),*]; $args:ident; [$($index:ident:$it:ident $pool:ident $at:literal $width:ident),*];)*) => {{
            let mut result = Vec::new();
            $(fixed!($variant, [$($definition)*], $name, $units, result);)*
            result
        }};
    }
    let opcodes = reseam_apk::reseam_dex::instruction_catalogue!(rows);
    let widths = opcodes
        .iter()
        .map(|op| format!("            {} -> {}", op.value, op.units))
        .collect::<Vec<_>>()
        .join("\n");
    let invokes = opcodes
        .iter()
        .filter(|op| op.invokes)
        .map(|op| op.value.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let returns = opcodes
        .iter()
        .filter(|op| op.returns)
        .map(|op| op.value.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let entries = opcodes
        .iter()
        .map(|op| format!("    {}(0x{:02x})", kotlin_opcode_name(op.name), op.value))
        .collect::<Vec<_>>()
        .join(",\n");
    let widths = format!(
        "// Generated from the DEX instruction catalogue. Do not edit.\npackage app.reseam.patch.dex\n\ninternal object OpcodeWidths {{\n    fun units(opcode: Int): Int =\n        when (opcode) {{\n{widths}\n            else -> error(\"unsupported opcode: $opcode\")\n        }}\n    fun invokes(opcode: Int): Boolean = when (opcode) {{ {invokes} -> true; else -> false }}\n    fun returns(opcode: Int): Boolean = when (opcode) {{ {returns} -> true; else -> false }}\n}}\n"
    );
    let enumeration = opcode_enum(&entries);
    (widths, enumeration)
}

fn opcode_enum(entries: &str) -> String {
    format!(
        r"// Generated from the DEX instruction catalogue. Do not edit.
package app.reseam.patch.dex

/** Android DEX opcode values. */
enum class Opcode(val value: Int) {{
{entries};

    val isInvoke: Boolean get() = OpcodeWidths.invokes(value)
    val isReturn: Boolean get() = OpcodeWidths.returns(value)
    /** Execution never falls through to the next instruction. */
    val endsFlow: Boolean get() = isReturn || this == THROW || this == GOTO || this == GOTO_16 || this == GOTO_32
    val isMoveResult: Boolean get() = this == MOVE_RESULT || this == MOVE_RESULT_WIDE || this == MOVE_RESULT_OBJECT

    companion object {{
        private val byValue = entries.associateBy {{ it.value }}
        fun of(value: Int): Opcode? = byValue[value]
    }}
}}
"
    )
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
