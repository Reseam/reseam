# Browser patcher

This is a basic UI over the existing Reseam SDK. Rust inspection, patch planning, DEX/resource editing, bundle verification and APK signing run in a WASI WebAssembly worker. CheerpJ 4.3 runs the existing Kotlin patch JARs with Java 17 in a separate worker. Generated BoltFFI bindings connect their native calls to the same Rust patch bridge. The browser runtime replaces only the generated native transport class: scalar 64-bit values cross CheerpJ as [one-element `long[]` arrays](https://cheerpj.com/docs/reference/CJ3Library#conversion-rules), preserving literals and mutation revisions without JavaScript Number rounding. Patch JARs and the rest of the Kotlin runtime remain unchanged.

## Build and run

Install the repository's pinned Rust toolchain, JDK 17, Bun 1.4.2, and [WASI SDK 33](https://github.com/WebAssembly/wasi-sdk/releases/tag/wasi-sdk-33). Install the exact BoltFFI CLI version from `.boltffi-version`.

From the engine repository:

```sh
cargo install boltffi_cli --version "=$(cat .boltffi-version)" --locked
bun install --cwd browser --frozen-lockfile
WASI_SDK_PATH=/path/to/wasi-sdk-33.0-x86_64-linux bun run --cwd browser engine
bun run --cwd browser build
bun run --cwd browser preview
```

`browser/dist` is the deployable static application. The engine build generates and packages the WASM module, Java host, shared Kotlin runtime and native-method manifest together. Runtime assets use a content-addressed directory and are checked for size and SHA-256 before loading. Publish the entire directory together; keep previous content-addressed assets available until older pages expire.

The engine uses the `browser` Cargo profile with thin LTO and one codegen unit. The WASI target enables SIMD for compression, decompression and checksums without changing compression levels or output bytes.

`bun run --cwd browser dev` rebuilds on changes and serves bundled workers. CheerpJ needs a classic worker, so development also uses Vite's production worker bundles.

## Hosting

Serve over HTTPS, with these headers on the HTML, assets and worker responses:

```text
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

Serve `.wasm` as `application/wasm`. Cache content-addressed assets as immutable and serve the HTML with revalidation. Vite's local preview config supplies the isolation headers; localhost is a secure context.

The browser must support WebAssembly SIMD, shared memory, workers, Web Locks, IndexedDB, and OPFS synchronous access handles inside workers. Chromium is the browser used for the real-app validation. The application checks essential capabilities before opening a session. Java runtime loading requires access to `https://cjrtnc.leaningtech.com/4.3/` and its runtime resources. Account for CheerpJ's WebAssembly and JavaScript compilation when setting a Content Security Policy.

## Files and signing

APK, APKM and XAPK inputs remain browser `File` objects. Additional split APKs and file/folder patch options are mounted under the session's virtual filesystem. Scratch files and outputs live in private OPFS storage. Cancellation closes the bridge and terminates the engine/JVM workers. Scratch files are reclaimed when their final descriptor closes, and disposing the session removes its storage directory. A later session removes abandoned directories after obtaining their Web Lock, so it cannot remove another active tab's files.

Signer approval is required before patch execution. Signed catalogs, dependencies, compatibility, option validation and failures use the existing SDK. Approvals and one atomic private-key/certificate record persist in IndexedDB. A Web Lock serializes signing-identity creation across tabs. Existing `.pk8`/`.der` pairs can be imported. Download and keep the identity backup to preserve updates across browser profiles or cleared site data.

Signing uses the same P-256 APK v2 signer as the CLI. It requires Android 7 or later; the engine does not add a v1 signature for older Android versions. Entropy comes from `crypto.getRandomValues`. The signer uses randomized ECDSA, so identical inputs and keys can produce different signing-block bytes even between two CLI runs.

## Compare real apps with the CLI

The comparison runner uses actual app files and the official signed bundle, the same selection/options, and the exact same private key and certificate in Chromium and the CLI. It compares whole APK hashes, every uncompressed ZIP entry, every physical byte outside the APK signing block, verifies the same signing certificate, and runs Android's `apksigner verify` on both outputs. It verifies APK v2 signatures for Android 7+ and also records the verifier's result for each APK's declared minimum SDK. An APK declaring Android below 7 requires v1 signing to pass that latter check; both hosts preserve the existing engine's v2-only behavior.

```sh
CARGO_BUILD_JOBS=4 cargo build --release -p reseam-cli
bun run --cwd browser build:test
MATRIX_PATH=/path/to/apps.json \
CHROMIUM_PATH=/path/to/chromium \
APKSIGNER=/path/to/android-sdk/build-tools/36.0.0/apksigner \
bun browser/scripts/compare-cli.ts
```

The matrix format is:

```json
{
  "bundle": "/path/to/official.reseam",
  "trust": "bundle signer public key in hex",
  "key": "/path/to/reseam.pk8",
  "cert": "/path/to/reseam.der",
  "apps": [
    {
      "id": "youtube",
      "apk": "/path/to/youtube.apk",
      "splits": [],
      "selection": { "preset": "recommended", "enable": [], "disable": [], "options": {} }
    }
  ]
}
```

The default selection is `recommended`. `OUTPUT_DIR`, `RESEAM_BIN`, `TEST_PORT` and `BROWSER_PROFILE` can override runner paths. `RESUME=1` retains successful cases and retries incomplete cases. Detailed outputs, patch metrics, and comparison results go under `browser/build/comparison`. `PROFILE_BRIDGE=1` records per-patch native-call counts and timings in `browser.log`; `PROFILE_CPU=1` writes Chromium worker CPU profiles to each case's `cpu-trace.json`. Both are disabled during the recorded matrix run. Private keys are mounted directly from disk and are not included in the report. Run `bun run --cwd browser build` again to produce the production application without the comparison hook.

## Recorded validation

[validation.json](validation.json) records the 2026-10-02 real-app run, input and output hashes, matching signing identities, and timings. All five supported apps passed with recommended patches and default options. Seven cases cover APK, APKM and separately supplied splits: two Reddit builds, X, Telegram, Instagram, and YouTube as both APK and APKM. All **149 APK pairs** have identical bytes outside the signing block and identical ZIP entries, including `META-INF`; every APK v2 signature verifies on Android 7+. The complete signed files differ because the signer randomizes ECDSA.

| Input | APK outputs | CLI SDK | Browser SDK | Browser patch wall time |
| --- | ---: | ---: | ---: | ---: |
| Reddit 2026.39, build 2639041, APKM | 34 | 1.39 s | 10.24 s | 11.23 s |
| X 12.29.1, APKM | 38 | 1.15 s | 11.38 s | 12.38 s |
| Telegram 12.7.1, APK | 1 | 0.52 s | 5.16 s | 6.14 s |
| Instagram 447, APK + splits | 5 | 1.71 s | 12.16 s | 13.13 s |
| YouTube 21.37.42, APK | 1 | 1.88 s | 16.20 s | 17.20 s |
| YouTube 21.37.42, APKM | 36 | 3.19 s | 20.30 s | 21.30 s |
| Reddit 2026.39, build 2639031, APKM | 34 | 1.40 s | 9.74 s | 10.73 s |

These are sequential workstation measurements in Chrome for Testing 145, with cached Java runtime downloads and bridge profiling disabled. SDK timings exclude Java runtime initialization; patch wall time includes it, and a cold network adds further download time. Native peak RSS stayed below 255 MiB; browser engine linear memory ranged from 78 to 230 MiB and excludes CheerpJ and browser overhead.

A separate before/after Telegram run used the same Chrome 145 executable, cached profile, APK, bundle, patch selection and signing identity. SDK time fell from **81.71 s to 5.16 s** (15.8×); patch execution fell from **63.23 s to 0.83 s**. Both outputs matched the CLI outside the signing block. These are single-run measurements.

Metadata-cache checks read an invocation-scoped Java `long[]` updated by each native response. Named method queries filter in Rust before crossing the Java bridge, including superclass resolution. Telegram's native method-metadata calls fell from 21,928 to 91. Input and scratch-file reads each use an 8 MiB cache shared across all files in the session; writes, truncation and removal invalidate scratch-file cache entries. Large reads remain streamed.

The second speed pass batches DEX header backpatches through the writer's existing 256 KiB window, retaining patch order and overlapping-write semantics. Instagram's output-writing phase fell from 11.96 s to 3.85 s, and Reddit's from 7.43 s to 2.25 s. A call-site cast query now executes in Rust, replacing 66,431 instruction bridge calls in Instagram with 272 batched queries. The optimized browser matched both the previous CLI (`e84abdd`) and the rebuilt CLI across all 149 APK pairs.

Relative to `e84abdd`, browser SDK times fell from 21.72 s to 12.16 s for Instagram, 15.05 s to 10.24 s for Reddit build 2639041, 13.98 s to 11.38 s for X, 7.62 s to 5.16 s for Telegram, and 19.62 s to 16.20 s for YouTube APK. The APKM and second Reddit cases are recorded in `validation.json`.

The earlier implementation also passed a separate YouTube Loop video run in Chrome for Testing 145. The current matrix applies all 90 recommended YouTube patches, including Loop video and its dependencies. The production UI also passed in Chrome 145 with gzip-compressed runtime assets. Its downloaded X base APK matched the CLI outside the signing block. The real UI workflow was checked for cancellation and retry, explicit signer approval, imported keys, downloading the patched base APK and identity backups, and persistence across reload. To repeat it against an app from the comparison matrix:

```sh
MATRIX_PATH=/path/to/apps.json APP_ID=x-12-29-1 \
CHROMIUM_PATH=/path/to/chromium bun browser/scripts/verify-ui.ts
```

The UI check also works with the production build. Output equivalence and Android signature verification do not establish on-device app behavior; no Android device run was performed.

## CheerpJ licensing

The UI includes Leaning Technologies' credit. This FOSS implementation loads CheerpJ from its CDN under the [Community License](https://cheerpj.com/docs/licensing). Self-hosting CheerpJ or use outside that license requires the relevant commercial license. Set `VITE_CHEERPJ_LICENSE_KEY` when building a deployment that uses a license key. It is a client runtime configuration value.

## Replacing the UI

Import `BrowserSession` from `src/session.ts`. Open it with mounted files and an optional abort signal, call `request('inspect', ...)`, then `request('patch', ...)`, and obtain downloadable `File` objects from `artifacts()`. Pass the same SDK request models used by native hosts. Events and structured `EngineError.problem` values are available to the host UI. One session permits one completed patch run. Keep it alive while its downloads are needed, then await `dispose()`. `wasmMemoryBytes` reports the engine's linear-memory allocation after a request; it does not include the JVM worker or the browser's total memory.
