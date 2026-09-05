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
| `--trust <PUBLIC_KEY_HEX>` | Repeatable. Bundle signer to accept. Same as `reseam patch`. |
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
| `ValidatePatches` | (dry-run only) Resolve the selection and check compatibility per patch. |
| `ApplyPatches` | Run each patch. |
| `WriteUnsignedArtifacts` | Serialize the patched APK or split set to a temp file. |
| `LoadSigningKey` | Load or generate the APK signing key. |
| `SignArtifacts` | Produce APK Signature Scheme v2 signatures. |

Per phase the report records duration in milliseconds and, where available, RSS, peak RSS, and peak native heap in bytes.

## Plain-text report

```
APK: app.apk
Bundle: patches.reseam
Splits: 0
Dry run: false
Warmups: 1
Measured runs: 5

iteration  1:      ok  total=4.12s  peak_rss=375.30MiB  final_rss=268.10MiB (anon=180.20MiB file=87.90MiB)  final_heap=22.40MiB  jvm_committed=96.00MiB
iteration  2:      ok  total=4.05s  peak_rss=372.80MiB  final_rss=266.90MiB (anon=179.60MiB file=87.30MiB)  final_heap=22.10MiB  jvm_committed=96.00MiB
...

summary: 5 ok, 0 failed
  total: min=4.01s median=4.05s max=4.12s mean=4056.0 ms
  peak rss: min=372.80MiB median=375.30MiB max=376.10MiB

phase breakdown:
  open_apk                 median=135ms max_peak_rss=120.40MiB max_peak_heap=15.20MiB
  load_bundles             median=630ms max_peak_rss=145.20MiB max_peak_heap=15.20MiB
  apply_patches            median=2.10s max_peak_rss=375.30MiB max_peak_heap=22.40MiB
  write_unsigned_artifacts median=680ms max_peak_rss=375.30MiB max_peak_heap=22.40MiB
  load_signing_key         median=12ms  max_peak_rss=375.30MiB max_peak_heap=22.40MiB
  sign_artifacts           median=240ms max_peak_rss=376.10MiB max_peak_heap=22.40MiB

apply_patches memory attribution (sampled at apply-phase peak):
  ...
```

A failed iteration prints `iteration  n:  failed  <error>` instead. Skipped phases (for example `validate_patches` outside `--dry-run`) are omitted. The attribution block at the end breaks the apply-phase peak down into materialized DEX structures, the JVM heap, and the rest of the native heap, from the last successful iteration.

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
      "metrics": { "total_duration_ms": 4120, "phases": [ ... ], "final_rss_bytes": ..., "peak_rss_bytes": ..., "apply_diagnostics": { ... } }
    }
  ],
  "summary": {
    "successful_iterations": 5,
    "failed_iterations": 0,
    "total_duration_ms": { "min": 4010, "median": 4050, "max": 4120, "mean": 4056.0 },
    "final_rss_bytes": { "min": ..., "median": ..., "max": ..., "mean": ... },
    "peak_rss_bytes": { "min": ..., "median": ..., "max": ..., "mean": ... },
    "phases": [
      {
        "phase": "apply_patches",
        "duration_ms": { "min": 2050, "median": 2100, "max": 2180, "mean": 2104.0 },
        "rss_bytes": { ... },
        "peak_rss_bytes": { ... },
        "heap_peak_bytes": { ... }
      }
    ]
  }
}
```

`metrics` is the engine's `PatchMetrics` record for that iteration, the same one Reseam Manager receives. `mean` is a float; `min`, `median`, and `max` are integers. Memory fields are omitted where the engine cannot read them.

## Tips

- Run with the release CLI for meaningful numbers: `cargo build --release -p reseam-cli`.
- Pin the APK and bundle. Substituting either between runs invalidates the comparison.
- For CI gates, parse the JSON. Failed iterations show up as `success: false` with `error` populated and the run exits non-zero.
- A single warmup is usually enough on cold caches. More warmups stabilize the median at the cost of wall time.
