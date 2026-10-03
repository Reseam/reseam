<p align="center">
  <img src="https://reseam.app/logo.svg" alt="Reseam logo" width="96">
</p>

<h1 align="center">reseam-apk</h1>

Reads and writes APKs for the Reseam engine: the ZIP container, the binary manifest and XML, the resource table, and the DEX files inside.

- Reads and writes APKs with the alignment and compression Android expects, streaming large entries instead of loading them into memory.
- Opens a base APK with its config splits, or an APKM or XAPK file, as one set of components.
- Parses and compiles Android binary XML (AXML), used by `AndroidManifest.xml` and compiled layouts.
- Parses and rewrites `resources.arsc`, including styled string pools.
- Moves `classes*.dex` in and out of [`reseam-dex`](../dex/).

## Modules

| Module | Contents |
|---|---|
| `apk_file` | `ApkFile` and `ApkComponent`: open, edit, and write an APK and its splits |
| `axml` | AXML reader, writer, and compiler, plus Android framework attribute IDs |
| `resources` | `ResourceTable` for `resources.arsc` |
| `entry` | entry-name rules: DEX ordinals, signature files, native libraries |

## Example

```rust
use reseam_apk::{ApkFile, reseam_dex::ParseOptions};

let apk = ApkFile::open("app.apk", ParseOptions::default())?;
let dex = apk.dex(); // every classes*.dex as one MultiDexContainer
```
