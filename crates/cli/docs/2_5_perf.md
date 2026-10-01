---
title: Perf
description: Benchmark a bundle against an APK and report phase timings and process memory.
---

# `reseam perf`

Each iteration runs the patch pipeline in a fresh child process and discards its output in a temporary directory. RSS peaks belong to that child, so earlier phases, warmups, and measured iterations cannot contaminate another iteration. SDK timings exclude child-process startup; opening inputs and starting the patch JVM remain part of the measured pipeline.

```bash
reseam perf app.apk --bundle patches.reseam --trust <PUBLIC_KEY_HEX> --warmup 1 --iterations 5 --json
```

The patch arguments match `reseam patch`, including splits, selection options, signing files, and `--dry-run`. `--iterations` defaults to 1 and must be positive; `--warmup` defaults to 0. Warmups can prime filesystem caches, but do not warm a shared JVM. A failed warmup aborts measurement. Failed measured iterations are reported and make the command exit unsuccessfully.

The phases are `open_apk`, `load_bundles`, `apply_patches`, `write_unsigned_artifacts`, `load_signing_key`, and `sign_artifacts`. Dry runs replace application and writing with `validate_patches`. Selection text is resolved against the already loaded specifications and package, without reopening inputs.

Phase `rss_bytes` values are endpoint samples. The legacy `peak_rss_bytes` field is the process high-water mark observed at that endpoint, which can have been established in an earlier phase. It is never an interval peak. The run's `peak_rss_bytes` is the child-process peak; apply diagnostics are sampled immediately after application. JVM used, committed, and maximum heap are reported separately: committed heap is not resident memory and cannot be subtracted from RSS. Allocator heap counters are unavailable and remain null in SDK metrics.

JSON reports contain `bundle_paths` as an array and one tagged result per iteration:

```json
{"iteration": 1, "status": "success", "metrics": {"total_duration_ms": 4120, "phases": []}}
{"iteration": 2, "status": "failure", "error": "..."}
```

`summary` aggregates successful iterations with minimum, median (the upper middle sample), maximum, and mean values. Missing memory observations are null. `metrics` uses the same schema as the application SDK.

Pin the APK, bundle, signing identity, and machine when comparing runs. Use an optimized CLI for timing comparisons; debug builds verify behavior but do not establish release performance.
