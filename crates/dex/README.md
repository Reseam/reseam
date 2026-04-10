# stitch-dex

DEX file parser, writer, and mutator. Handles the full Dalvik Executable format: reading all sections (header, string/type/proto/field/method IDs, class defs, code items, annotations, debug info, encoded values), mutating the in-memory representation, and writing valid DEX files back out with correct checksums and offsets.

## Key capabilities

- **Parse** DEX files from bytes or memory-mapped files into a full `DexFile` representation
- **Multi-DEX** support via `MultiDexContainer` for APKs with multiple `classes*.dex` files
- **Write** modified `DexFile` back to bytes with proper section ordering, string sorting, and checksum/signature computation
- **Fingerprinting** — pattern-based method matching using `Fingerprint` and `OpcodeMatcher` for locating injection points without hardcoding offsets
- **Lookup tables** for fast class/method/field resolution by name
- **MUTF-8 and LEB128** encoding/decoding

## Modules

| Module | Purpose |
|--------|---------|
| `read` | DEX binary parser — header, IDs, class data, code items, annotations, debug info |
| `write` | DEX binary writer — section layout, sorting, compaction, instruction encoding |
| `types` | Data structures for all DEX sections (classes, methods, fields, annotations, etc.) |
| `file` | `DexFile` API — class ops, interning, fingerprinting, search, lookup tables |
| `encoding` | MUTF-8 string encoding and LEB128 integer encoding |
| `util` | Shared helpers |

## Usage

```rust
use stitch_dex::{parse, write, ParseOptions};

// Round-trip: parse and rewrite
let bytes = std::fs::read("classes.dex")?;
let mut dex = parse(&bytes, ParseOptions::default())?;
let output = write(&mut dex)?;
```

```rust
use stitch_dex::{DexFile, Fingerprint, FingerprintBuilder, InstructionPattern};

// Find methods by opcode pattern
let fingerprint = FingerprintBuilder::new()
    .opcodes(vec![InstructionPattern::Opcode(0x6e)]) // invoke-virtual
    .build();
let matches = dex.find_methods(&fingerprint);
```
