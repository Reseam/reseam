<p align="center">
  <img src="https://reseam.app/logo.svg" alt="Reseam logo" width="96">
</p>

<h1 align="center">reseam-dex</h1>

Reads, edits, and writes Android DEX files. The rest of the Reseam engine uses it for every bytecode change a patch makes.

- Parses DEX files from bytes or memory-mapped files into a `DexFile`.
- Writes a modified `DexFile` back out with correct section order, offsets, and checksums.
- Finds methods by the strings they load or an opcode pattern, through a reference index.
- Decodes MUTF-8 strings and LEB128 numbers, with strict options that reject malformed input.

## Modules

| Module | Contents |
|---|---|
| `model` | data types for every DEX section and id |
| `instruction` | the `Instruction` set, its catalogue, operands, branches, and the raw instruction stream |
| `read` | the parser: header, IDs, class data, code, annotations, debug info |
| `file` | `DexFile`: file-backed tables, interning, class edits, undo |
| `search` | the reference index, method scans, and opcode patterns |
| `edit` | method body edits: relocation, control flow, liveness, register growth, and blocks written over temporaries that insertion places in registers |
| `write` | the writer: section layout, pool ordering, splitting, compaction |
| `encoding` | MUTF-8 and LEB128 |

## Example

```rust
use reseam_dex::{InstructionPattern, ParseOptions, parse_file, write_spooled};

let dex = parse_file("classes.dex", ParseOptions::default())?;

let prompts = dex.find_methods_by_strings(&["rate_prompt_shown"])?;
let invokes = dex.find_methods_with_opcodes(&[InstructionPattern::OpcodeValue(0x6e)])?; // invoke-virtual

let (output, _) = write_spooled(&dex, None)?;
std::io::copy(&mut output.reader(), &mut std::fs::File::create("classes-rewritten.dex")?)?;
```
