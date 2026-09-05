# reseam-sign

APK signing implementation supporting Android Signature Scheme v2.

## Key capabilities

- **V2 signing**: ECDSA-SHA256 chunk-based signing per the APK Signature Scheme v2 spec, either into a new buffer or in place on an unsigned file
- **Key generation**: generate ECDSA P-256 signing keys with self-signed X.509 certificates, and save them as PKCS#8 and DER
- **PKCS#8/X.509 loading**: load existing private keys (DER) and certificates
- **Signing block manipulation**: parse, inject, and reconstruct APK signing blocks

## Modules

| Module | Purpose |
|--------|---------|
| `v2` | APK Signature Scheme v2 implementation |
| `signing_block` | APK signing block parsing and construction |

Key loading and certificate construction are internal; `SigningKey` and `GeneratedKey` are the public entry points.

## Usage

```rust
use reseam_sign::{GeneratedKey, SigningKey, v2};

let generated = GeneratedKey::generate()?;
generated.save("out.pk8".as_ref(), "out.der".as_ref())?;
let key = SigningKey::from_files("out.pk8".as_ref(), "out.der".as_ref())?;
v2::sign_file_in_place(&unsigned_apk_file, &key)?;
```

`v2::sign` returns a signed copy instead of rewriting the file.
