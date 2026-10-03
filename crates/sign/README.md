<p align="center">
  <img src="https://reseam.app/logo.svg" alt="Reseam logo" width="96">
</p>

<h1 align="center">reseam-sign</h1>

Signs APKs for the Reseam engine with APK Signature Scheme v2 (ECDSA P-256, SHA-256). Android 7 and later accept it.

- Signs into a new buffer, or in place on a file without loading the whole APK into memory.
- Generates a P-256 key with a self-signed certificate, saved as PKCS#8 and DER.
- Loads an existing key and certificate, and rejects a certificate that doesn't match the key.

## Example

```rust
use reseam_sign::{SigningKey, v2};

let key = SigningKey::load_or_generate("out.pk8".as_ref(), "out.der".as_ref())?;
v2::sign_file_in_place(&unsigned_apk_file, &key)?;
```

`load_or_generate` reuses an existing pair and creates one only when both files are missing. If only one of the two exists, it fails and names the missing file. It never overwrites a key.

`sign_file_in_place` rewrites the file it is given. Pass a file nothing else uses: if it fails halfway, the file is left incomplete. `v2::sign` returns a signed copy instead.

Certificate dates and issuers are not checked. Android treats the certificate as the app's signing identity, not as proof of trust.
