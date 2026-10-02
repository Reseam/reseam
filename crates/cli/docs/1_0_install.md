---
title: Install
description: Download or build the reseam binary.
---

# Install

## Prebuilt binary

Grab the latest release from the Forgejo releases page:

<https://git.reseam.app/reseam/reseam/releases>

Each release ships two binaries: `reseam-linux-x64` (Linux, x86-64) and `reseam-windows-x64.exe` (Windows, x86-64). Both need a **64-bit** Java runtime, version 17 or newer, which the CLI finds through `JAVA_HOME` or `java` on your `PATH`.

On Linux, rename the binary and put it on your `PATH`:

```bash
chmod +x reseam-linux-x64
mv reseam-linux-x64 ~/.local/bin/reseam
```

On Windows, rename it to `reseam.exe` and put it in a folder on your `PATH`.
Set `JAVA_HOME` to the runtime's root if Java is not on `PATH`:

```powershell
$env:JAVA_HOME = "C:\path\to\jdk-21"
.\reseam.exe --version
```

Other platforms build from source.

## From source

The CLI is a workspace crate. Install the Rust toolchain pinned by
`rust-toolchain.toml`, a JDK in `JAVA_HOME`, and the pinned BoltFFI CLI from
`.boltffi-version`. Generate the bridge and embedded patch runtime first:

```bash
cargo xtask regen patch-api
cargo xtask runtime
```

Then, from the repo root:

```bash
cargo build --release -p reseam-cli
```

The binary lands at `target/release/reseam` on Linux or `target/release/reseam.exe` on Windows. Native Windows builds also need the C toolchain for their Rust target (Visual Studio for MSVC, or llvm-mingw for GNU LLVM). See the [SDK build guide](../../../sdk/README.md#build) for Linux-to-Windows cross-compilation and target JDK configuration. To install into `~/.cargo/bin`:

```bash
cargo install --path crates/cli
```

## Check it works

```bash
reseam --help
reseam patch --help
```

Every subcommand supports `--help`.

## Logging

Logs go to stderr. By default, Reseam shows info and errors, including patch progress and the final summary. Warnings, debug messages, trace messages, and crate targets are hidden.

Set `RUST_LOG` to control log levels and show crate targets. For example, include warnings with `RUST_LOG=info`, or dig into one crate:

```bash
RUST_LOG=reseam_patcher=debug reseam patch app.apk --bundle patches.reseam
```
