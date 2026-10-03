# reseam-dex

DEX file parser, writer, and mutator. Reads and writes the DEX sections Reseam currently supports (header, string/type/proto/field/method IDs, class defs, code items, annotations, debug info, encoded values), mutates the in-memory representation, and writes valid DEX files back out with correct checksums and offsets.

## Key capabilities

- **Parse** DEX files from bytes or memory-mapped files into a full `DexFile` representation
- **Multi-DEX** support via `MultiDexContainer` for APKs with multiple `classes*.dex` files
- **Write** modified `DexFile` back to bytes with proper section ordering, string sorting, and checksum/signature computation
- **Strict parsing options** for MUTF-8 and LEB128 decoding when malformed input should be rejected rather than normalized
- **Fingerprinting**: find methods by strings, literals, types, access flags, and opcode patterns with `Fingerprint`, without hardcoding offsets
- **Lookup tables** for fast class/method/field resolution by name
- **MUTF-8 and LEB128** encoding/decoding

## Modules

| Module | Purpose |
|--------|---------|
| `read` | DEX binary parser: header, IDs, class data, code items, annotations, debug info |
| `write` | DEX binary writer: section layout, sorting, compaction, instruction encoding |
| `types` | Data structures for all DEX sections (classes, methods, fields, annotations, etc.) |
| `file` | `DexFile` API: class ops, interning, fingerprinting, search, lookup tables |
| `encoding` | MUTF-8 string encoding and LEB128 integer encoding |
| `util` | Shared helpers |

## Usage

```rust
use reseam_dex::{parse, write, ParseOptions};

// Round-trip: parse and rewrite
let bytes = std::fs::read("classes.dex")?;
let dex = parse(&bytes, ParseOptions::default())?;
let output = write(&dex)?;
```

```rust
use reseam_dex::{Fingerprint, InstructionPattern};

// Methods that load "rate_prompt_shown" and call invoke-virtual
let fingerprint = Fingerprint {
    strings: Some(vec!["rate_prompt_shown".into()]),
    opcodes: Some(vec![InstructionPattern::OpcodeValue(0x6e)]),
    ..Fingerprint::default()
};
let hits = dex.find_methods_by_fingerprint(&fingerprint)?;
```
