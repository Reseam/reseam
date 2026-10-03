# @reseam/browser

The Reseam SDK for web pages. Rust inspection, patching and signing run in a WASI worker; CheerpJ 4.3 runs the unchanged Kotlin patch JARs on Java 17 in a second worker, calling into Rust through the generated BoltFFI bindings. 64-bit values cross CheerpJ as one-element `long[]` arrays, because its JavaScript conversion rounds scalar longs through `Number`. The package has no UI; `reseam.app/patch` hosts it.

## Build

Needs the pinned Rust toolchain, JDK 17, Bun 1.4.2, [WASI SDK 33](https://github.com/WebAssembly/wasi-sdk/releases/tag/wasi-sdk-33) and the BoltFFI CLI from `.boltffi-version`.

```sh
cargo install boltffi_cli --version "=$(cat .boltffi-version)" --locked
bun install --cwd browser --frozen-lockfile
WASI_SDK_PATH=/path/to/wasi-sdk-33.0-x86_64-linux bun run --cwd browser engine
bun run --cwd browser types
```

`engine` writes the WASM modules, Java host, Kotlin runtime and native-method manifest to `public/runtime/<hash>/` and their sizes and SHA-256 sums to `src/runtime.generated.ts`, which the loader checks. `types` writes declarations to `types/`. Release tags publish the package to `https://git.reseam.app/api/packages/reseam/npm/`.

## Host requirements

The package ships TypeScript sources and their workers for a Vite host to bundle. The host must:

- bundle workers as classic scripts (`worker: { format: 'iife' }`): CheerpJ loads through `importScripts`;
- keep the package out of dependency pre-bundling and compile it for SSR (`optimizeDeps.exclude`, `ssr.noExternal`), so worker URLs resolve;
- serve `public/runtime/` as immutable files and pass its URL to `JavaRuntime` as `runtimeBase`, keeping old hashes until pages that use them expire;
- send `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp` on the page and every worker script;
- allow `https://cjrtnc.leaningtech.com/4.3/`, and WebAssembly compilation in its Content Security Policy.

Browsers need WebAssembly SIMD, shared memory, Web Locks, IndexedDB and OPFS sync access handles in workers.

## Use

```ts
import { BrowserSession, JavaRuntime } from '@reseam/browser';

const java = new JavaRuntime({ runtimeBase });
void java.warmup();
const session = await BrowserSession.open(files, { javaRuntime: java, signal, onEvent });
const inspection = await session.inspect({ apk_path, split_paths, bundle_paths });
await session.mount(signingFilesAndOptionFiles);
const outcome = await session.patch(request);
const artifacts = await session.artifacts();
await session.dispose();
```

Requests and results are the SDK models (`src/models.ts`). Failures are `EngineError`s carrying the SDK `Problem`.

- A session runs one patch. Inspection keeps the opened APK and verified catalogs for it; trust and payload hashes are checked again before patch code loads.
- `mount()` adds files after inspection but cannot replace inspected inputs.
- Artifacts are OPFS files: keep the session open until they are saved, because `dispose()` deletes them.
- One `JavaRuntime` can serve every session. `warmup()` starts Java without loading patch code; a cold start takes seconds. A canceled run discards the JVM.
- `compressionWorkers` (1 to 4, default 2) sets how many workers deflate DEX entries.

Inputs stay browser `File`s. Scratch files are deleted once no name or descriptor refers to them, and a later session reclaims a crashed tab's storage after taking its Web Lock.

Signing is the CLI's APK v2 signer: P-256, Android 7 and later, no v1. With empty key paths the engine generates a pair, which the host should store and offer as a backup. ECDSA is randomized, so signing blocks differ between runs with the same key. Signer approval and key storage belong to the host.

## Compare with the CLI

`compare-cli.ts` patches real apps in Chromium and with the CLI using the same bundle, selection, key and certificate. It then checks that every byte outside the signing block and every ZIP entry match, and that `apksigner` verifies both.

```sh
CARGO_BUILD_JOBS=4 cargo build --release -p reseam-cli
bun run --cwd browser harness
MATRIX_PATH=apps.json CHROMIUM_PATH=/path/to/chromium \
APKSIGNER=/path/to/build-tools/36.0.0/apksigner bun browser/scripts/compare-cli.ts
```

```json
{
  "bundle": "/path/to/official.reseam",
  "trust": "signer public key, hex",
  "key": "/path/to/reseam.pk8",
  "cert": "/path/to/reseam.der",
  "apps": [{ "id": "youtube", "apk": "/path/to/youtube.apk", "splits": [] }]
}
```

Each app may add a `selection`; the default is `recommended`. `OUTPUT_DIR`, `RESEAM_BIN`, `TEST_PORT` and `BROWSER_PROFILE` override paths, and `RESUME=1` reruns only failed cases. `PROFILE_BRIDGE=1` logs per-patch native-call counts and times; `PROFILE_CPU=1` saves Chromium CPU profiles.

## Validation

[validation.json](validation.json) holds the run below: recommended patches, default options, the same key and certificate for both hosts, Chrome for Testing 145, Java already started. All 149 APK pairs match the CLI outside the signing block, with identical ZIP entries and valid signatures.

| Input | APKs | CLI | Browser |
| --- | ---: | ---: | ---: |
| Reddit 2026.39 (2639041), APKM | 34 | 1.4s | 7.6s |
| X 12.29.1, APKM | 38 | 1.2s | 9.3s |
| Telegram 12.7.1, APK | 1 | 0.5s | 4.7s |
| Instagram 447, APK + splits | 5 | 1.6s | 10.5s |
| YouTube 21.37.42, APK | 1 | 1.8s | 15.5s |
| YouTube 21.37.42, APKM | 36 | 2.1s | 17.0s |
| Reddit 2026.39 (2639031), APKM | 34 | 1.4s | 7.6s |

Firefox-based browsers take about twice as long (YouTube APK: 43s), mostly running patch code in CheerpJ. Matching bytes do not prove on-device behavior; no device run is recorded.

## CheerpJ license

CheerpJ loads from its CDN under the [Community License](https://cheerpj.com/docs/licensing), which requires credit; hosts show it. Self-hosting needs the commercial license. Pass `licenseKey` to `JavaRuntime` when a deployment has one.
