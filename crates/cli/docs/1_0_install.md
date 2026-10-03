---
title: Install
description: Download the reseam binary or build it from source.
---

# Install

## Download

Get the latest release from the [Download](/download/) page. There are two builds:

- `reseam-linux-x64` for Linux on x86-64,
- `reseam-windows-x64.exe` for Windows on x86-64.

Both run patches on Java, so you also need a **64-bit Java runtime, version 17 or newer**. The CLI finds it through `JAVA_HOME`, or `java` on your `PATH`.

**Linux:** rename the file and put it on your `PATH`:

```bash
chmod +x reseam-linux-x64
mv reseam-linux-x64 ~/.local/bin/reseam
reseam --version
```

**Windows:** rename it to `reseam.exe` and put it in a folder on your `PATH`. If Java isn't on your `PATH`, set `JAVA_HOME`:

```powershell
$env:JAVA_HOME = "C:\path\to\jdk-21"
reseam --version
```

On other platforms, build from source.

## Build from source

You need the Rust toolchain pinned in `rust-toolchain.toml`, a JDK in `JAVA_HOME`, and the BoltFFI CLI at the version in `.boltffi-version`. From the engine repository:

```bash
cargo xtask regen patch-api
cargo xtask runtime
cargo build --release -p reseam-cli
```

The binary is `target/release/reseam`. Cross-compiling for Windows from Linux is covered in [`sdk/README.md`](https://git.reseam.app/reseam/reseam/src/branch/main/sdk/README.md#build).

## Logs

Logs go to stderr. By default you see progress, the result of each patch, and errors. Set `RUST_LOG` for more:

```bash
RUST_LOG=debug reseam patch app.apk --bundle patches.reseam --trust <key>
```

`RESEAM_JVM_HEAP` sets the memory limit of the Java runtime that runs patches (default `256m`).
