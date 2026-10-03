---
title: Perf
description: Measure how long a patch run takes and how much memory it uses.
---

# `reseam perf`

Runs a patch several times and reports the time and memory of each step. Useful when a patch makes the run slow.

```bash
reseam perf app.apk --bundle patches.reseam --trust <key> --warmup 1 --iterations 5
```

It takes the same flags as [`reseam patch`](2_0_patch.md) to choose patches, plus:

| Flag | |
|---|---|
| `--iterations <n>` | measured runs (default 1) |
| `--warmup <n>` | runs before measuring, not counted (default 0) |
| `--json` | print the report as JSON |

Each run happens in a fresh process and writes to a temporary folder, so runs don't affect each other. The report shows, for each run and as min, median, max, and mean:

- total time and peak memory,
- each step: opening the APK, loading bundles, checking patches, applying them, writing the APK, and signing.

If any measured run fails, the report still prints, and the command fails at the end.
