# stitch-cli

Command-line interface for Stitch. Provides the `stitch` binary.

## Commands

### `stitch patch`

Apply a patch bundle to an APK.

```
stitch patch app.apk --bundle patches/ --output patched.apk
```

Options:
- `--key` / `--cert` — sign with an existing PKCS#8 key and X.509 cert (auto-generates if omitted)
- `--enable` / `--disable` — toggle specific patches by name
- `--option PATCH.KEY=VALUE` — set patch options
- `--dry-run` — resolve and validate without applying

### `stitch list`

List all patches in a bundle with their metadata and compatibility info.

```
stitch list patches/
```

### `stitch info`

Print APK metadata: package name, version, DEX file count, and split/component info.

```
stitch info app.apk
```
