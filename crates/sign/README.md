<p align="center">
  <img src="https://reseam.app/logo.svg" alt="Reseam logo" width="96">
</p>

<h1 align="center">reseam-sign</h1>

Signs APKs for the Reseam engine with APK Signature Scheme v2 (ECDSA P-256, SHA-256). Android 7 and later accept it.

- Signs in place on a file without loading the whole APK into memory.
- Generates a P-256 key with a self-signed certificate, saved as PKCS#8 and DER.
- Loads an existing key and certificate, and rejects a certificate that doesn't match the key. A PKCS#8 key without its public point, as Java writes keys, takes the point from the certificate.
- Reads and writes PKCS#12 keystores, the format Reseam apps back keys up in. Reading also takes the 3DES and RC2 encryption older Android writes.
- Keeps the signing block entries of the source APK that aren't signatures, such as Play's frosting, which apps can check at runtime.

## Example

```rust
use reseam_sign::{PreservedPairs, SigningKey, v2};

let preserved = PreservedPairs::from_apk(&source_apk_bytes)?;
let key = SigningKey::load_or_generate("out.pk8".as_ref(), "out.der".as_ref())?;
v2::sign_file_in_place(&unsigned_apk_file, &key, &preserved)?;
```

`load_or_generate` reuses an existing pair and creates one only when both files are missing. If only one of the two exists, it fails and names the missing file. It never overwrites a key.

`PreservedPairs::from_apk` reads the source's signing block and drops the v2 and v3 signatures, source stamps and padding, since those don't hold for the new contents. Read it before writing the unsigned output, since the source may be gone by the time you sign.

`sign_file_in_place` rewrites the file it is given. Pass a file nothing else uses: if it fails halfway, the file is left incomplete.

Certificate dates and issuers are not checked. Android treats the certificate as the app's signing identity, not as proof of trust.
