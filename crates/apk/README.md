# stitch-apk

APK file reader and writer. Handles the ZIP container, Android Binary XML (AXML), resource tables, and DEX extraction/injection.

## Key capabilities

- **Read/write APK files** as ZIP archives with proper alignment and compression
- **Parse and compile AXML** (Android Binary XML) — the binary format used for `AndroidManifest.xml` and other compiled XML resources
- **Resource table** parsing for resolving resource IDs to values
- **DEX extraction** — pull `classes*.dex` from an APK into `stitch-dex` structures
- **DEX injection** — write modified DEX files back into an APK
- **Multi-DEX** aware across all operations

## Modules

| Module | Purpose |
|--------|---------|
| `apk_file` | `ApkFile` — high-level APK abstraction for reading, modifying, and writing APKs |
| `zip` | ZIP reader/writer handling APK-specific alignment requirements |
| `axml` | AXML reader, writer, and compiler for Android binary XML |
| `dex` | DEX extraction and injection helpers bridging APK ↔ `stitch-dex` |
| `resources` | Android resource table (`resources.arsc`) parser |

## Usage

```rust
use stitch_apk::{ApkFile, ApkReader};

let apk = ApkFile::open("app.apk")?;
let manifest = apk.manifest(); // parsed AXML
let dex_files = apk.dex(); // MultiDexContainer
```
