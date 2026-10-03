<p align="center">
  <img src="https://reseam.app/logo.svg" alt="Reseam logo" width="96">
</p>

<h1 align="center">Reseam</h1>

<p align="center">
  <a href="https://reseam.app">Website</a> ·
  <a href="https://reseam.app/docs/">Docs</a> ·
  <a href="https://reseam.app/download/">Download</a>
</p>

Reseam applies community-built patches to Android apps. This repository is the engine behind it: the code that reads an app, runs patches on it, and signs the result. Reseam Manager, the `reseam` CLI, and the browser patcher on reseam.app all use it.

APK parsing, DEX editing, and signing are written in Rust. Patches are written in Kotlin. They run on the JVM on desktop, on Android's runtime on phones, and on CheerpJ in the browser.

Docs for patch authors and the CLI are at [reseam.app/docs](https://reseam.app/docs/). Their sources are [`docs/`](docs/) and [`crates/cli/docs/`](crates/cli/docs/).

## Layout

| Path | |
|---|---|
| `crates/storage` | file-backed bytes and temporary storage |
| `crates/dex` | DEX parser, editor, and writer |
| `crates/apk` | APKs, split sets, APKM/XAPK, manifest, resources |
| `crates/sign` | APK Signature Scheme v2 signing |
| `crates/model` | requests, results, events, and errors shared by every host |
| `crates/patcher` | bundles, patch planning and execution, the Kotlin bridge |
| `crates/cli` | the `reseam` command |
| `sdk/` | the application SDK, its BoltFFI bindings (`sdk/native`), and its WASI build (`sdk/browser`) |
| `sdk-kotlin/` | the `reseam-sdk` Kotlin package for Android and JVM apps |
| `patch-api/` | the `reseam-patch-sdk` Kotlin API patches are written against |
| `gradle-plugin/` | the Gradle plugin that builds bundles |
| `browser/` | `@reseam/browser`, the engine for web pages |
| `xtask/` | code generation, packaging, and releases (`cargo xtask`) |

## Build

You need the Rust toolchain in `rust-toolchain.toml`, JDK 17 in `JAVA_HOME`, and the BoltFFI CLI at the version in `.boltffi-version`.

```bash
cargo xtask regen patch-api   # generate the Kotlin bridge
cargo xtask runtime           # build the patch runtime jar the engine embeds
cargo build --release -p reseam-cli
```

Building the Android and desktop SDK needs more toolchains; see [`sdk/README.md`](sdk/README.md#build). The browser package has its own steps in [`browser/README.md`](browser/README.md).

## Checks

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
./gradlew spotlessCheck checkKotlinAbi
```

Code style is in [`STYLE.md`](STYLE.md). How the engine and the patch API talk to each other is in [`docs/internals/boltffi.md`](docs/internals/boltffi.md).

## Release

```bash
cargo xtask release <version>
git push --follow-tags
```

This sets the version, commits, and tags `v<version>`. CI then publishes the CLI, both Kotlin packages, and `@reseam/browser` at that version.

## License

GPL-3.0-or-later.
