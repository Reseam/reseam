---
title: Inspect
description: Print APK metadata.
---

# `reseam info`

Opens a single APK, parses its manifest, and prints a summary.

```bash
reseam info app.apk
```

Output:

```
APK: app.apk
  package:    com.example.app
  version:    1.4.2
  versionCode: 14200
  dex files:  12
  components: 1
  classes:    24183
  methods:    192445
```

Missing fields (for example, an APK without a version name) are omitted.

`reseam info` takes one APK positional argument and does not accept `--split`. To look at a split APK set, open the base APK on its own, or run `reseam patch --dry-run` which opens the full set.

| Argument | Purpose |
|----------|---------|
| `<apk>` | APK path to inspect. |

To inspect a bundle instead of an APK, see [`reseam bundle list`](3_0_bundle.md).
