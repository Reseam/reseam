# reseam-sign

APK signing implementation supporting Android Signature Scheme v2.

## Key capabilities

- **V2 signing**: ECDSA-SHA256 chunk-based signing per the APK Signature Scheme v2 spec, either into a new buffer or in place on a file
- **Key generation**: generate ECDSA P-256 signing keys with self-signed X.509 certificates, and save them as PKCS#8 and DER
- **PKCS#8/X.509 loading**: load existing private keys (DER) and certificates
- **Signing block manipulation**: parse, inject, and reconstruct APK signing blocks

## Modules

| Module | Purpose |
|--------|---------|
| `v2` | APK Signature Scheme v2 implementation |
| `signing_block` | ZIP and signing-block location, and borrowed pair lookup |

Key loading and certificate construction are internal; `SigningKey` owns the private key and its matching certificate.

## Usage

```rust
use reseam_sign::{SigningKey, v2};

let key = SigningKey::load_or_generate("out.pk8".as_ref(), "out.der".as_ref())?;
v2::sign_file_in_place(&unsigned_apk_file, &key)?;
```

`v2::sign` returns a signed copy instead of rewriting the file.

`from_pkcs8` and `from_files` reject malformed certificates and certificates whose subject public
key differs from the P-256 private key. Certificate bytes are preserved; dates and issuer trust
are not checked because Android uses the certificate as an application signing identity.

`load_or_generate` reuses an existing pair, generates when both files are absent, and rejects a
partial pair. Both parent directories must exist. `save` publishes a pair without overwriting
existing files. Each file is staged in its destination directory and published with
`persist_noclobber`. A partial pair names the missing file and requires restoring it or deleting
the other file before generating a new pair. There is no journal or automatic recovery.

File signing reads only the ZIP footer into memory, hashes file-backed 1 MiB chunks in parallel
(with at most eight live mappings), and moves the central directory through a 1 MiB buffer.
The APK and its central directory are never buffered as a whole. Use an exclusively held temporary
file: an I/O failure while moving the directory can leave incomplete output, and the caller decides
when to flush and publish it. In-memory signing remains available for already resident inputs.
