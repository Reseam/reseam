// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt::Write as _;
use std::path::PathBuf;

// BytecodeList.def does not record which operands an instruction writes. By Hermes convention
// a leading register is the destination; these follow the v98 interpreter (lib/VM/Interpreter.cpp
// and Interpreter-slowpaths.cpp).
const READS_FIRST: &[&str] = &[
    "FastArrayStore",
    "FastArrayPush",
    "FastArrayAppend",
    "CacheNewObject",
    "StoreToEnvironment",
    "StoreToEnvironmentL",
    "StoreNPToEnvironment",
    "StoreNPToEnvironmentL",
    "PutByIdLoose",
    "PutByIdStrict",
    "PutByIdLooseLong",
    "PutByIdStrictLong",
    "TryPutByIdLoose",
    "TryPutByIdStrict",
    "TryPutByIdLooseLong",
    "TryPutByIdStrictLong",
    "PutOwnBySlotIdx",
    "PutOwnBySlotIdxLong",
    "DefineOwnById",
    "DefineOwnByIdLong",
    "DefineOwnByIndex",
    "DefineOwnByIndexL",
    "DefineOwnInDenseArray",
    "DefineOwnInDenseArrayL",
    "DefineOwnByVal",
    "DefineOwnGetterSetterByVal",
    "PutByValLoose",
    "PutByValStrict",
    "PutByValWithReceiver",
    "AddOwnPrivateBySym",
    "PutOwnPrivateBySym",
    "Ret",
    "Throw",
    "ThrowIfThisInitialized",
    "IteratorClose",
    "UIntSwitchImm",
    "StringSwitchImm",
];

const ALSO_WRITES: &[(&str, &[usize])] = &[
    ("CreateBaseClass", &[1]),
    ("CreateBaseClassLongIndex", &[1]),
    ("CreateDerivedClass", &[1]),
    ("CreateDerivedClassLongIndex", &[1]),
    ("IteratorBegin", &[1]),
    ("IteratorNext", &[1]),
    ("GetPNameList", &[1, 2, 3]),
    ("GetNextPName", &[3]),
];

// Hermes names the wider encodings of an instruction by suffixing its base name.
const WIDTH_SUFFIXES: &[&str] = &["LongIndex", "Long", "Short", "L"];

struct Definition {
    name: String,
    operands: Vec<String>,
    ids: Vec<&'static str>,
}

impl Definition {
    fn new(name: String, operands: Vec<String>) -> Self {
        let ids = vec!["None"; operands.len()];
        Self {
            name,
            operands,
            ids,
        }
    }

    fn family(&self) -> &str {
        WIDTH_SUFFIXES
            .iter()
            .find_map(|suffix| self.name.strip_suffix(suffix))
            .unwrap_or(&self.name)
    }

    fn widens(&self, narrow: &Self) -> bool {
        self.name != narrow.name
            && self.family() == narrow.family()
            && self.operands.len() == narrow.operands.len()
            && self.ids == narrow.ids
            && self
                .operands
                .iter()
                .zip(&narrow.operands)
                .all(|(wide, narrow)| widening(narrow).contains(&wide.as_str()))
    }

    fn size(&self) -> usize {
        self.operands.iter().map(|o| width(o)).sum()
    }
}

fn widening(kind: &str) -> &'static [&'static str] {
    match kind {
        "Reg8" => &["Reg8", "Reg32"],
        "Reg32" => &["Reg32"],
        "UInt8" => &["UInt8", "UInt16", "UInt32"],
        "UInt16" => &["UInt16", "UInt32"],
        "UInt32" => &["UInt32"],
        "Addr8" => &["Addr8", "Addr32"],
        "Addr32" => &["Addr32"],
        "Imm32" => &["Imm32"],
        "Double" => &["Double"],
        _ => panic!("unknown operand kind {kind}"),
    }
}

fn width(kind: &str) -> usize {
    match kind {
        "Reg8" | "UInt8" | "Addr8" => 1,
        "UInt16" => 2,
        "Double" => 8,
        _ => 4,
    }
}

fn main() {
    println!("cargo:rerun-if-changed=vendor/v98/BytecodeList.def");
    println!("cargo:rerun-if-changed=vendor/v98/Builtins.def");
    let definitions = definitions(
        &std::fs::read_to_string("vendor/v98/BytecodeList.def")
            .expect("vendored v98 opcode definition exists"),
    );
    assert!(definitions.len() <= 256);
    let index = |name: &str| {
        definitions
            .iter()
            .position(|d| d.name == name)
            .unwrap_or_else(|| panic!("{name} is a v98 opcode"))
    };
    let mut writes: Vec<Vec<usize>> = definitions
        .iter()
        .map(|d| {
            Vec::from_iter(
                d.operands
                    .first()
                    .filter(|o| o.starts_with("Reg"))
                    .map(|_| 0),
            )
        })
        .collect();
    for name in READS_FIRST {
        writes[index(name)].clear();
    }
    for (name, operands) in ALSO_WRITES {
        let definition = &definitions[index(name)];
        for &operand in *operands {
            assert!(definition.operands[operand].starts_with("Reg"));
        }
        writes[index(name)].extend_from_slice(operands);
    }

    let mut output = String::from(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n#[repr(u8)]\npub(crate) enum Op {\n",
    );
    for definition in &definitions {
        writeln!(output, "    {},", definition.name).expect("writing to a string succeeds");
    }
    output.push_str("}\n\n");
    writeln!(output, "static V98: [Opcode; {}] = [", definitions.len())
        .expect("writing to a string succeeds");
    for (definition, writes) in definitions.iter().zip(&writes) {
        let mut wider: Vec<_> = definitions
            .iter()
            .filter(|d| d.widens(definition))
            .collect();
        wider.sort_by_key(|d| d.size());
        write!(
            output,
            "    Opcode {{ op: Op::{}, operands: &[",
            definition.name
        )
        .expect("writing to a string succeeds");
        for (operand, id) in definition.operands.iter().zip(&definition.ids) {
            write!(
                output,
                "Operand {{ kind: OperandKind::{operand}, id: IdKind::{id} }}, "
            )
            .expect("writing to a string succeeds");
        }
        write!(output, "], writes: &{writes:?}, wider: &[").expect("writing to a string succeeds");
        for wide in wider {
            write!(output, "Op::{}, ", wide.name).expect("writing to a string succeeds");
        }
        output.push_str("] },\n");
    }
    output.push_str("];\n");

    let builtins = std::fs::read_to_string("vendor/v98/Builtins.def")
        .expect("vendored v98 builtin definition exists");
    let mut index = 0_u8;
    for line in builtins.lines().map(str::trim) {
        if [
            "NORMAL_METHOD(",
            "BUILTIN_METHOD(",
            "PRIVATE_BUILTIN(",
            "JS_BUILTIN(",
        ]
        .iter()
        .any(|prefix| line.starts_with(prefix))
        {
            let args = arguments(line);
            if line == "PRIVATE_BUILTIN(apply)" || line == "PRIVATE_BUILTIN(applyArguments)" {
                writeln!(
                    output,
                    "pub(crate) const BUILTIN_{}: u8 = {index};",
                    args[0].to_uppercase()
                )
                .expect("writing to a string succeeds");
            }
            index += 1;
        }
    }
    let path = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    std::fs::write(path.join("opcodes.rs"), output).expect("generated opcode table is writable");
}

fn definitions(source: &str) -> Vec<Definition> {
    let mut definitions: Vec<Definition> = Vec::new();
    for line in source.lines().map(str::trim) {
        if line.contains('\\') {
            continue;
        }
        if line.starts_with("DEFINE_OPCODE_") {
            let args = arguments(line);
            if let Some((name, operands)) = args.split_first() {
                definitions.push(Definition::new(name.clone(), operands.to_vec()));
            }
        } else if let Some(rest) = line.strip_prefix("DEFINE_JUMP_") {
            let args = arguments(line);
            if let [name] = &args[..] {
                let count: usize = rest[..1].parse().expect("jump operand count");
                for (suffix, address) in [("", "Addr8"), ("Long", "Addr32")] {
                    let mut operands = vec![address.to_owned()];
                    operands.extend(std::iter::repeat_n("Reg8".to_owned(), count - 1));
                    definitions.push(Definition::new(format!("{name}{suffix}"), operands));
                }
            }
        } else if line.starts_with("OPERAND_") {
            let args = arguments(line);
            if let [name, number] = &args[..] {
                let kind = match line.split('_').nth(1).expect("operand macro kind") {
                    "STRING" => "String",
                    "FUNCTION" => "Function",
                    "BIGINT" => "BigInt",
                    "REGEXP" => "RegExp",
                    "SHAPE" => "Shape",
                    "VALUE" => "ValueBuffer",
                    "SWITCH" => "Switch",
                    other => panic!("unknown ID macro {other}"),
                };
                let definition = definitions
                    .iter_mut()
                    .find(|d| &d.name == name)
                    .expect("ID annotation follows its opcode");
                let number: usize = number.parse().expect("ID operand number");
                definition.ids[number - 1] = kind;
            }
        }
    }
    definitions
}

fn arguments(line: &str) -> Vec<String> {
    let Some((_, args)) = line.split_once('(') else {
        return Vec::new();
    };
    let Some((args, _)) = args.split_once(')') else {
        return Vec::new();
    };
    args.split(',').map(|s| s.trim().to_owned()).collect()
}
