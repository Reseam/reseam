<p align="center">
  <img src="https://reseam.app/logo.svg" alt="Reseam logo" width="96">
</p>

<h1 align="center">reseam-dex</h1>

Reads, edits, and writes Android DEX files. The rest of the Reseam engine uses it for every bytecode change a patch makes.

- Parses DEX files from bytes or memory-mapped files into a `DexFile`.
- Opens every `classes*.dex` of an app as one `MultiDexContainer`.
- Writes a modified `DexFile` back out with correct section order, offsets, and checksums.
- Finds methods with a `Fingerprint`: strings, literals, types, access flags, names, and opcode patterns.
- Decodes MUTF-8 strings and LEB128 numbers, with strict options that reject malformed input.

## Modules

| Module | Contents |
|---|---|
| `read` | the parser: header, IDs, class data, code, annotations, debug info |
| `write` | the writer: section layout, sorting, compaction, instruction encoding |
| `types` | data types for every DEX section |
| `file` | `DexFile`: class edits, interning, fingerprints, search, lookup tables |
| `encoding` | MUTF-8 and LEB128 |

## Example

```rust
use reseam_dex::{Fingerprint, InstructionPattern, ParseOptions, parse, write};

let bytes = std::fs::read("classes.dex")?;
let dex = parse(&bytes, ParseOptions::default())?;

let fingerprint = Fingerprint {
    strings: Some(vec!["rate_prompt_shown".into()]),
    opcodes: Some(vec![InstructionPattern::OpcodeValue(0x6e)]), // invoke-virtual
    ..Fingerprint::default()
};
let hits = dex.find_methods_by_fingerprint(&fingerprint)?;

let output = write(&dex)?;
```
