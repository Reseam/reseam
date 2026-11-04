---
description: Release a bundle so users can add it to Reseam Manager and get updates.
---

# Publishing

A published bundle is two files on any static host:

- the `.reseam` bundle,
- `patches.json`, an index of your releases, their download links, and your public key.

Users add your bundle from a link you share. Reseam Manager then checks `patches.json` for new releases, and updates the bundle on its own.

## Release with the template

The template's `.github/workflows/release.yml` publishes a release whenever you push a tag that starts with `v`. It runs on GitHub, and on Forgejo, including Codeberg.

1. Add your signing key as the repository secret `BUNDLE_SIGNING_KEY_B64`:

   ```bash
   base64 -w0 ~/.reseam/bundle-signing.key
   ```

2. On Forgejo, set the repository variable `RUNNER` to the label of a runner with Java and Node.js. GitHub uses `ubuntu-latest`.

3. Push a tag:

   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

The workflow builds and signs the bundle with the CLI that matches your plugin version, then runs [`reseam release patches`](/docs/cli/release/). That adds the release to the `patches.json` of your latest release, with the subjects of the commits since the previous tag as its notes. It publishes both files on a release whose notes start with your share link.

## Release by hand

`reseam release` can also publish to an S3-compatible bucket, or to a folder you upload to any static host:

```bash
./gradlew bundle -PreleaseTag=v0.1.0
reseam release patches build/reseam/my-patches.reseam --tag v0.1.0 \
  --to ./site --public-url https://example.com/bundles
```

It reads the `patches.json` already in the target, adds the release, and writes the bundle to `<target>/v0.1.0/` and the index to `<target>/patches.json`. Upload the folder as it is: each release keeps its own folder and URL. `--keep 3` keeps only the three newest releases. See [`reseam release`](/docs/cli/release/) for S3 and every flag.

## Sign apart from the build

Packing runs your bundle's code, and signing needs your key. When someone else's code builds on your machines, as in a shared release pipeline, keep the two apart so the key is never on the machine that runs the code:

```bash
./gradlew unsignedBundle -PreleaseTag=v0.1.0
reseam bundle sign build/reseam/unsigned/my-patches.reseam \
  --key ~/.reseam/bundle-signing.key --out my-patches.reseam
```

Run the first command where the code builds, without the key, and the second where the key is. [`reseam bundle sign`](/docs/cli/bundle/#reseam-bundle-sign) runs none of the bundle's code, and it checks the bundle as loading one does before it signs.

## Share your bundle

Share this link, with the address of your repository or of your `patches.json` encoded into it:

```text
https://reseam.app/add/?url=<encoded patches.json URL>
```

On Android, the link opens Reseam Manager, which shows the bundle's apps, patches, and key, and asks before adding it. Without Reseam Manager, or on a computer, it opens a page with the steps and a download link. The link only carries your URL: Reseam Manager downloads `patches.json` and the bundle from your host.

Releases from the template list this link already. To encode a URL yourself:

```bash
jq -rn --arg url https://example.com/bundles/patches.json '$url | @uri'
```

Users can also paste the `patches.json` URL into Add patch source in Reseam Manager. For a repository address, Reseam Manager looks for `patches.json` in the repository's latest release, at the paths GitHub, Forgejo, Gitea, and GitLab serve release files from.

## Your key

Users trust your public key, not your name. When they add your bundle, Reseam Manager shows the key and asks before trusting it.

- Publish your public key somewhere users already trust you, such as your repository's README, so they can compare it.
- Back up the key file. A bundle signed with a new key asks every user to trust it again.
- `reseam release patches` refuses to change the key recorded in a published `patches.json`.

## When Reseam updates

Bundles only load on engines of their own release line. When Reseam moves to a new line, bump the plugin version in `settings.gradle.kts`, fix anything the new patch API changed, rebuild, and release. Until you do, the newer Reseam Manager can't load your bundle, and tells its users so.
