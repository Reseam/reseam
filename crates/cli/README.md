<p align="center">
  <img src="https://reseam.app/logo.svg" alt="Reseam logo" width="96">
</p>

<h1 align="center">reseam-cli</h1>

The `reseam` command. Patch authors use it to build, sign, test, and publish patch bundles. Anyone can use it to patch an APK on a computer without Reseam Manager.

```bash
reseam patch app.apk --bundle patches.reseam --trust <PUBLIC_KEY_HEX>
```

The CLI trusts no bundle signer on its own. Pass `--trust` with the public key of every signer you accept; a bundle signed by anyone else is refused before its code runs.

| Command | What it does |
|---|---|
| `reseam patch` | Applies patches from bundles to an APK, APKM, XAPK, or split set, and signs the result. |
| `reseam perf` | Patches several times and reports time and memory per step. |
| `reseam bundle` | Creates signing keys, packs bundles, checks a staging manifest, and lists a bundle's patches. |
| `reseam publish` | Adds a release to a `patches.json` or `manager.json` index. |
| `reseam info` | Prints an app's package, version, and size. |

Install steps and every flag are in the [CLI docs](https://reseam.app/docs/cli/overview/) (source: [`docs/`](docs/)). Run any command with `--help` for a short summary.

Build it from the workspace root:

```bash
cargo xtask regen patch-api
cargo xtask runtime
cargo build --release -p reseam-cli
```
