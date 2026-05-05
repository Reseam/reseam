---
title: Perf
description: Benchmark a bundle against an APK and report per-phase timings.
---

# `reseam perf`

Runs the real patch pipeline N times into a temporary location and prints how long each phase took. Use it to compare bundle revisions or to spot regressions during development.

```bash
reseam perf app.apk --bundle patches.reseam --warmup 1 --iterations 5
```

The output APK is written into a `tempfile::tempdir()` per iteration and discarded; nothing lands on the working tree. The patch arguments are the same as `reseam patch`.

## Arguments

| Argument | Purpose |
|----------|---------|
| `<apk>` | Base APK path. |
| `--bundle <PATH>` | Signed `.reseam` bundle. Verified on open. |
| `--split <APK>` | Repeatable split APK input. |
| `--key <PK8>` | PKCS#8 signing key. Requires `--cert`. |
| `--cert <DER>` | DER X.509 certificate. Requires `--key`. |
| `--enable <PATCH>` | Repeatable. Force a patch on. |
| `--disable <PATCH>` | Repeatable. Force a patch off. |
| `--option PATCH.KEY=VALUE` | Repeatable patch option. |
| `--dry-run` | Run validation only; skip apply, write, and sign. |
| `--iterations <N>` | Measured runs. Default `1`. Must be greater than `0`. |
| `--warmup <N>` | Unmeasured runs before measurement. Default `0`. |
| `--json` | Print machine-readable JSON. Without it the report is plain text. |

A failed warmup aborts the run before any measured iteration starts. A measured iteration that fails is included in the summary, and the command exits non-zero at the end.

## Phases

Each iteration is broken down by phase, in order:

| Phase | What it covers |
|-------|----------------|
| `OpenApk` | Open the base APK and any splits for patching. |
| `LoadBundles` | Read each `.reseam`, verify signatures, instantiate patches. |
| `CompileSelection` | Resolve enable/disable/options into a concrete plan. |
| `ValidatePatches` | (dry-run only) Check compatibility per patch. |
| `ApplyPatches` | Run each patch. |
| `WriteUnsignedArtifacts` | Serialize the patched APK or split set to a temp file. |
| `LoadSigningKey` | Load or generate the APK signing key. |
| `SignArtifacts` | Produce APK Signature Scheme v2 signatures. |

Per phase the report records duration in milliseconds and, where available, RSS and peak RSS in bytes.

## Plain-text report

```
APK: app.apk
Bundle: patches.reseam
Splits: 0
Dry run: false
Warmups: 1
Measured runs: 5

iteration  1:      ok  total=1.42s  peak_rss=412.30MiB  final_rss=388.10MiB
iteration  2:      ok  total=1.31s  peak_rss=410.80MiB  final_rss=387.20MiB
...

summary: 5 ok, 0 failed
  total: min=1.30s median=1.34s max=1.42s mean=1340.0 ms
  peak rss: min=410.80MiB median=412.30MiB max=414.10MiB

phase breakdown:
  open_apk                 median=82ms  max_peak=120.40MiB
  load_bundles             median=63ms  max_peak=145.20MiB
  compile_selection        median=4ms   max_peak=145.20MiB
  apply_patches            median=720ms max_peak=410.80MiB
  write_unsigned_artifacts median=180ms max_peak=412.30MiB
  load_signing_key         median=12ms  max_peak=412.30MiB
  sign_artifacts           median=240ms max_peak=414.10MiB
```

Skipped phases (for example `validate_patches` outside `--dry-run`) are omitted.

## JSON report

```bash
reseam perf app.apk --bundle patches.reseam --iterations 5 --json > perf.json
```

```json
{
  "apk_path": "app.apk",
  "bundle_path": "patches.reseam",
  "split_count": 0,
  "dry_run": false,
  "warmup_iterations": 1,
  "measured_iterations": 5,
  "iterations": [
    {
      "iteration": 1,
      "success": true,
      "error": null,
      "metrics": { "total_duration_ms": 1420, "phases": [ ... ], "final_rss_bytes": ..., "peak_rss_bytes": ... }
    }
  ],
  "summary": {
    "successful_iterations": 5,
    "failed_iterations": 0,
    "total_duration_ms": { "min": 1300, "median": 1340, "max": 1420, "mean": 1340.0 },
    "peak_rss_bytes": { "min": 430718976, "median": 432498688, "max": 434176000, "mean": 432490291.2 },
    "phases": [
      {
        "phase": "apply_patches",
        "duration_ms": { "min": 700, "median": 720, "max": 740, "mean": 720.0 },
        "peak_rss_bytes": { "min": ..., "median": ..., "max": ..., "mean": ... }
      }
    ]
  }
}
```

`mean` is a float; `min`, `median`, and `max` are integers. RSS fields are omitted on platforms where the engine cannot read them.

## Tips

- Run with the release CLI for meaningful numbers: `cargo build --release -p reseam-cli`.
- Pin the APK and bundle. Substituting either between runs invalidates the comparison.
- For CI gates, parse the JSON. Failed iterations show up as `success: false` with `error` populated and the run exits non-zero.
- A single warmup is usually enough on cold caches. More warmups stabilize the median at the cost of wall time.
