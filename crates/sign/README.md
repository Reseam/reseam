# stitch-sign

APK signing implementation supporting Android Signature Scheme v2 and v3.

## Key capabilities

- **V2 signing** — ECDSA-SHA256 chunk-based signing per the APK Signature Scheme v2 spec
- **V3 signing** — extends v2 with SDK version targeting and key rotation support
- **Key generation** — generate ECDSA P-256 signing keys and self-signed X.509 certificates
- **PKCS#8/X.509 loading** — load existing private keys (DER) and certificates
- **Signing block manipulation** — parse, inject, and reconstruct APK signing blocks

## Modules

| Module | Purpose |
|--------|---------|
| `v2` | APK Signature Scheme v2 implementation |
| `v3` | APK Signature Scheme v3 (v2 + SDK range and rotation) |
| `keystore` | Key generation (`GeneratedKey`) and loading (`SigningKey`) |
| `signing_block` | APK signing block parsing and construction |
| `der` | Minimal DER encoding for X.509 certificate construction |

## Usage

```rust
use stitch_sign::{GeneratedKey, v3};

let key = GeneratedKey::generate()?;
let signed_apk = v3::sign(&apk_bytes, &key.signing_key)?;
```
