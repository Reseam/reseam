# Agents

Follow `STYLE.md`. Its checks must pass before a change is done.

- `patch-api/generated` is gitignored: run `cargo xtask regen patch-api` after changing an exported JNI
  function, or the patcher build script fails.
- Builds share one machine: `CARGO_BUILD_JOBS=4`, and never run two builds at once.
- Use `cargo test --workspace --no-fail-fast`; without it cargo stops at the first failing test binary.
