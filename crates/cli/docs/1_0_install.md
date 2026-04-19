---
title: Install
description: Download or build the reseam binary.
---

# Install

## Prebuilt binary

Grab the latest release from the Forgejo releases page:

<https://git.reseam.app/reseam/reseam/releases>

Download the archive for your platform, extract `reseam`, and put it on your `PATH`:

```bash
chmod +x reseam
mv reseam ~/.local/bin/
```

## From source

The CLI is a workspace crate. From the repo root:

```bash
cargo build --release -p reseam-cli
```

The binary lands at `target/release/reseam`. To install into `~/.cargo/bin`:

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

Logs go to stderr through `tracing-subscriber`. The default filter is:

```
reseam=info,reseam_cli=info,reseam_patcher=info,reseam_apk=info,reseam_sign=info
```

Override with `RUST_LOG` to dig into one crate:

```bash
RUST_LOG=reseam_patcher=debug reseam patch app.apk --bundle patches.reseam
```
