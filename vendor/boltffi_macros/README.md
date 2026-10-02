# BoltFFI macro compatibility patch

This is the published `boltffi_macros` 0.31.0 source, licensed under MIT. The sole source change in `src/expansion/wrapper/export.rs` permits native ABI wrappers on WASI, in addition to non-WASM targets. Reseam hosts those wrappers in its browser worker and reuses BoltFFI's generated Kotlin codecs and JNI glue. BoltFFI's usual web wrappers remain enabled on WASM targets.

The dependency is pinned to keep generated bindings reproducible. Remove this override when upstream supports exposing the native ABI on WASI. The manifest omits upstream test targets whose fixtures are not included here.
