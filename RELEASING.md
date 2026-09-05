# Releasing

Three repositories release on tags, in this order: engine, then patches, then manager. Patches and the Manager download the engine release they are pinned to, so the engine goes first and must finish before the others start. The API and website have no release tags; redeploy them when they change.

## 1. Engine

Publishes the CLI (`reseam-linux-x64`) as a release asset and the SDKs (`app.reseam:reseam-patch-sdk`, `app.reseam:reseam-sdk`) to the Maven registry, all at one version.

```bash
cargo xtask release 0.3.1
git push --follow-tags
```

`cargo xtask release` sets the version in `Cargo.toml`, commits, and tags `v0.3.1`. CI refuses a tag that does not match the manifest, so tag only through this command. Secrets: `RELEASE_TOKEN`, `FORGEJO_PACKAGES_TOKEN`.

Version rule: patch for fixes, minor for new capability, and while the major is 0 a minor bump is a breaking change. Bundles record the engine version that packed them and load only on the same line (same minor while the major is 0, same major after). If a release breaks the bridge or the SDK wire format, bump the line and do steps 2 and 3 the same day, or every user sees "update Reseam" against a bundle that no longer loads.

## 2. Patches

Builds and signs the bundle with the pinned engine, writes `patches.json`, uploads both. Bundle versions are independent of the engine's.

1. If the engine line changed, update `ENGINE_VERSION` in `.forgejo/workflows/release.yml` and the `reseam-patch-sdk` version in `apps/*/patch/build.gradle.kts`, and rebuild locally once against the new SDK.
2. Tag and push:

```bash
git tag v0.2.0
git push --follow-tags
```

The API serves the newest release through `releases/download/latest`, so users see it within `CACHE_TTL` (five minutes by default). Secrets: `RELEASE_TOKEN`, `BUNDLE_SIGNING_KEY_B64` (the raw 32-byte seed, base64).

## 3. Manager

Builds signed APKs per ABI plus deb and rpm, writes `manager.json`, uploads everything. Installed apps read `manager.json` on launch and offer the download, so users learn about the release the same day.

1. If the engine line changed, update `ENGINE_VERSION` in `.forgejo/workflows/build.yml` and `reseam` in `gradle/libs.versions.toml`.
2. Tag and push; the tag becomes the app version:

```bash
git tag v1.0.1
git push --follow-tags
```

Secrets: `RELEASE_TOKEN`, `ANDROID_KEYSTORE_B64` (`base64 -w0 keystore.jks`), `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD`. Always the same keystore: Android refuses an update signed by a different key.

## 4. API and website

```bash
cd api && docker compose up -d --build
```

The website is rebuilt with `bun run build`; it pulls the documentation from the engine, CLI, and API repositories at build time, so rebuild it after doc changes.

## Checking a release

- Engine: the release page has `reseam-linux-x64`; `reseam --version` prints the tag.
- Patches: `https://api.reseam.app/v1/patches/version` shows the new version; `reseam bundle list <bundle> --trust <key>` shows `engine:` on the expected line.
- Manager: `https://api.reseam.app/v1/manager/version` shows the new version; a phone on the previous version shows the update banner on launch.
