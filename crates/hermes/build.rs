use std::fmt::Write as _;
use std::path::PathBuf;

#[derive(Clone)]
struct Definition {
    name: String,
    operands: Vec<String>,
    ids: Vec<String>,
}

fn main() {
    println!("cargo:rerun-if-changed=vendor/v98/BytecodeList.def");
    let source = std::fs::read_to_string("vendor/v98/BytecodeList.def")
        .expect("vendored v98 opcode definition exists");
    let mut definitions: Vec<Definition> = Vec::new();
    for line in source.lines().map(str::trim) {
        if line.starts_with("DEFINE_OPCODE_") && !line.contains('\\') {
            let args = arguments(line);
            if !args.is_empty() {
                definitions.push(Definition {
                    name: args[0].clone(),
                    operands: args[1..].to_vec(),
                    ids: vec!["None".to_owned(); args.len() - 1],
                });
            }
        } else if line.starts_with("DEFINE_JUMP_") && !line.contains('\\') {
            let args = arguments(line);
            if args.len() == 1 {
                let count: usize = line[12..13].parse().expect("jump operand count");
                for (suffix, address) in [("", "Addr8"), ("Long", "Addr32")] {
                    let mut operands = vec![address.to_owned()];
                    operands.extend(std::iter::repeat_n("Reg8".to_owned(), count - 1));
                    definitions.push(Definition {
                        name: format!("{}{suffix}", args[0]),
                        ids: vec!["None".to_owned(); count],
                        operands,
                    });
                }
            }
        } else if line.starts_with("OPERAND_") {
            let args = arguments(line);
            if args.len() == 2 {
                let kind = match line.split('_').nth(1).expect("operand macro kind") {
                    "STRING" => "String",
                    "FUNCTION" => "Function",
                    "BIGINT" => "BigInt",
                    "REGEXP" => "RegExp",
                    "SHAPE" => "Shape",
                    "VALUE" => "ValueBuffer",
                    "SWITCH" => "Switch",
                    _ => panic!("unknown ID macro"),
                };
                let definition = definitions
                    .iter_mut()
                    .find(|d| d.name == args[0])
                    .expect("ID annotation follows its opcode");
                let index: usize = args[1].parse().expect("ID operand number");
                kind.clone_into(&mut definition.ids[index - 1]);
            }
        }
    }
    assert!(definitions.len() <= 256);
    let mut output = String::from("pub static V98: &[Opcode] = &[\n");
    for definition in definitions {
        write!(
            output,
            "Opcode {{ name: {:?}, operands: &[",
            definition.name
        )
        .expect("writing to a string succeeds");
        for (operand, id) in definition.operands.iter().zip(definition.ids) {
            write!(
                output,
                "Operand {{ kind: OperandKind::{operand}, id: IdKind::{id} }},"
            )
            .expect("writing to a string succeeds");
        }
        output.push_str("] },\n");
    }
    output.push_str("];\n");
    let path = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    std::fs::write(path.join("opcodes.rs"), output).expect("generated opcode table is writable");
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
