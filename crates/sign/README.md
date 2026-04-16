# reseam-sign

APK signing implementation supporting Android Signature Scheme v2.

## Key capabilities

- **V2 signing** — ECDSA-SHA256 chunk-based signing per the APK Signature Scheme v2 spec
- **Key generation** — generate ECDSA P-256 signing keys and self-signed X.509 certificates
- **PKCS#8/X.509 loading** — load existing private keys (DER) and certificates
- **Signing block manipulation** — parse, inject, and reconstruct APK signing blocks

## Modules

| Module | Purpose |
|--------|---------|
| `v2` | APK Signature Scheme v2 implementation |
| `v3` | Reserved module name; currently returns `Unsupported` until a correct v3 implementation exists |
| `keystore` | Key generation (`GeneratedKey`) and loading (`SigningKey`) |
| `signing_block` | APK signing block parsing and construction |
| `der` | Minimal DER encoding for X.509 certificate construction |

## Usage

```rust
use reseam_sign::{GeneratedKey, v2};

let key = GeneratedKey::generate()?;
let signed_apk = v2::sign(&apk_bytes, &key.signing_key)?;
```

`reseam_sign::v3` is intentionally unavailable for real signing today; it returns `Unsupported` until the signed-data layout is implemented and verified correctly.
