# Manifest reference

`manifest.toml` at the bundle root.

```toml
[bundle]
name = "my-bundle"
author = "Your Name"
description = "One-line description"
format_version = 1
```

- **`name`** (required). Short identifier for the bundle. Lowercase, no spaces. Shown by `reseam bundle list` and embedded in `patches.json`.
- **`author`** (required). The publisher's name or handle. Shown to users in the Manager so they know who built the bundle.
- **`description`** (required). One line describing what the bundle contains.
- **`format_version`** (required). Bundle format version. Currently `1`. The engine refuses to load bundles whose `format_version` it doesn't recognize.

Per-patch metadata lives in the patch code itself, not here. Release metadata (version, download URL) lives in the `patches.json` release index produced at publish time.

See [build and publish](6_build_and_publish.md) for signing and packing.
