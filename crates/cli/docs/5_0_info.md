---
title: Info
description: Print an app's package, version, and DEX, class, and method counts.
---

# `reseam info`

```bash
reseam info app.apk
```

Prints what Reseam reads from an app: its name, package, version name and code, and how many DEX files, APK components, classes, and methods it has. Fields the app doesn't have are left out. It also opens `.apkm` and `.xapk` files.

To see what is in a bundle instead, use [`reseam bundle list`](3_0_bundle.md#reseam-bundle-list).
