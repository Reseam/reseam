# Style

Formatting and most rules are enforced by tooling. This file covers what tooling cannot check.

## Checks

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
./gradlew spotlessCheck :reseam-patch-sdk:checkKotlinAbi
```

The toolchain is pinned in `rust-toolchain.toml`, lints in `[workspace.lints]`, formatting in
`rustfmt.toml` (Rust) and Spotless with ktfmt (Kotlin). `patch-api/api/reseam-patch-sdk.api` is the
patch authoring API: a change to it is a change patch authors see, so it is reviewed as one
(`./gradlew :reseam-patch-sdk:updateKotlinAbi` after an intended change).

## Structure

- A crate owns one layer: `dex` the DEX format, `apk` APK containers, archives and Android resources,
  `sign` signing, `patcher` patch planning and the JNI bridge, `sdk` the entry point hosts call,
  `model` types shared across the host boundary. A lower layer never knows about a higher one.
- A module does one job. Parsing, editing and serializing a format are separate modules over one model.
- One implementation per rule. Two code paths that must agree (reader and writer, Rust and Kotlin,
  scan and full parse) share the definition they agree on.
- No trait, generic parameter or builder without two real users. No wrapper that only forwards.

## State and ownership

- Each piece of mutable state has one owner, and it changes only through that owner's methods, which
  keep its invariants (dirty flags, caches, indices). Types with invariants have no public mutable fields.
- Caches are invalidated by the operation that makes them stale, not by callers.
- Handles given across the JNI boundary are stable identities, never positions in a vector that moves.
- No global mutable state outside the JNI bridge, which holds the current patch run for callbacks.

## Errors

- Each library crate has one error type in `error.rs`, built with `thiserror`, carrying what failed and
  where (entry, offset, class, resource). `anyhow` is for binaries (`cli`, `xtask`) only.
- A fallible result is propagated with `?`. `.ok()`, `unwrap_or_default()` and `let _ =` on a
  `Result` are used only where absence is a valid answer, never to hide a failure.
- Input that Android accepts must parse. Input Reseam cannot handle is an error, not silently rewritten.
- Library code does not panic on input. `expect` is for invariants the code itself guarantees, with
  the invariant as the message.

## Types

- Typed models for every data flow: newtypes for indices and ids, enums for kinds and modes. No string
  keys, delimiter-joined identities, tuples of more than two values, or `bool` flags that select behavior.
- Text conversion happens at the boundary (CLI arguments, Kotlin API, XML text); inside, values stay typed.

## Rust

- Edition 2024 idioms: `let … else`, `if let` chains, `LazyLock`/`OnceLock`, `impl Trait` arguments,
  iterator adapters over index loops, `#[expect(lint, reason = "…")]` over `#[allow]`.
- `unsafe` only for FFI and memory maps, each block with a `// SAFETY:` line.
- Memory: DEX and APK data stay file-backed and streamed. Never read a whole APK, DEX or resource table
  into memory when a range will do, and never clone a large buffer. Peak RSS on the test apps stays at or
  under 300 MB.
- Use a well-maintained crate instead of hand-rolling a format or algorithm it covers, if it builds on
  Linux, Windows and Android.

## Comments

- Code says what it does. A comment says why, only when the reason is not visible in the code: a
  platform quirk, a format rule, an invariant. No section headers, no narration, no restating the code.
- Doc comments on the patch authoring API (`patch-api`), and on public items whose contract the
  signature does not show.

## Tests

A test earns its place by failing when behavior a user depends on breaks. Delete a test that:
- cannot fail: asserts a constant, a table against its own copy, that a value built one line earlier
  has the fields it was built with, or only that code runs without panicking;
- pins incidental detail: exact error text, internal layout, private helper output;
- duplicates another test, or guards one past bug without covering the general rule it broke.

Consolidate: tests of one behavior over several inputs become one table-driven test. Shared fixtures
build inputs; tests do not each hand-assemble binary data. Test through public interfaces with realistic
input. For refactors of the DEX and APK paths, output equivalence on real apps (`reseam patch` before
and after, entries compared outside `META-INF`) is the check.

## Kotlin

- ktfmt (kotlinlang style). The public API of `patch-api` is small and deliberate; everything else is
  `internal`.
- Generated bindings (`app.reseam.patch.native`) never appear in public signatures of `patch-api`.

## Commits

One line, conventional commits (`fix(dex): …`, `refactor(apk): …`), no body.
