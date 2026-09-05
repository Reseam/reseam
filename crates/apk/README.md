# reseam-apk

APK file reader and writer. Handles the ZIP container, Android Binary XML (AXML), resource tables, and DEX extraction and injection.

## Key capabilities

- **Read and write APK files** as ZIP archives with the alignment and compression Android expects, streaming large entries instead of holding them in memory
- **Split APKs**: a base APK plus config splits opened as one session of components, each with its own manifest, resources, and injected files
- **Parse and compile AXML**, the binary format used for `AndroidManifest.xml` and other compiled XML resources
- **Resource table** parsing and rewriting for `resources.arsc`, including styled string pools
- **DEX extraction and injection**: `classes*.dex` in and out of `reseam-dex` structures, multi-DEX aware
- **Scratch directories** that outlive a crashed process only until the next run sweeps them

## Modules

| Module | Purpose |
|--------|---------|
| `apk_file` | `ApkFile` and `ApkComponent`: open, modify, and write APKs and their splits |
| `zip` | ZIP reader and writer for APK-specific alignment and streaming |
| `axml` | AXML reader, writer, and compiler, plus the framework attribute ids |
| `resources` | `ResourceTable` for `resources.arsc` |
| `entry` | Entry-name rules: DEX ordinals, signature entries, native libraries |
| `scratch` | `ScratchDir`, per-process temporary directories |

## Usage

```rust
use reseam_apk::{ApkFile, reseam_dex::ParseOptions};

let apk = ApkFile::open("app.apk", &ParseOptions::default())?;
let dex = apk.dex(); // MultiDexContainer over every classes*.dex
```
