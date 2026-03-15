# WORK_LOG.md — dex-rs Implementation Progress

## What This Is
Implementing the ENTIRE dex-rs library from SPEC.md in one session. The spec is a ~2400-line document describing a production-grade DEX parser/writer in Rust with feature parity to dexlib2.

## Current State: FEATURE-COMPLETE, DUAL-APK BENCHMARKED ✓

### ALL 24 DEX files pass round-trip (parse → write → re-parse):

**YouTube APK (7 DEX, 44MB):**
```
classes.dex:  8326692 → 8328240 bytes (OK)
classes2.dex: 7884716 → 7887576 bytes (OK)
classes3.dex: 7743816 → 7746132 bytes (OK)
classes4.dex: 7853188 → 7854412 bytes (OK)
classes5.dex: 7675884 → 7677560 bytes (OK)
classes6.dex: 6669700 → 6672612 bytes (OK)
classes7.dex: 588640  → 588952 bytes  (OK)
```

**Instagram APK (17 DEX, 124MB):**
```
classes.dex:   9613036 → 9612376 bytes (OK)
classes2.dex: 10365132 → 10367596 bytes (OK)
classes3.dex:  9279392 → 9279064 bytes (OK)
classes4.dex:  8829940 → 8830688 bytes (OK)
classes5.dex:  9143868 → 9144068 bytes (OK)
classes6.dex:  8700328 → 8694984 bytes (OK)
classes7.dex:  4916068 → 4905480 bytes (OK)
classes8.dex:  5781728 → 5780784 bytes (OK)
classes9.dex:  6624676 → 6622272 bytes (OK)
classes10.dex: 7818200 → 7816476 bytes (OK)
classes11.dex: 7559284 → 7557496 bytes (OK)
classes12.dex: 9499172 → 9499072 bytes (OK)
classes13.dex: 3921676 → 3925400 bytes (OK)
classes14.dex: 8650628 → 8662020 bytes (OK)
classes15.dex: 6619900 → 6616552 bytes (OK)
classes16.dex: 7090096 → 7087528 bytes (OK)
classes17.dex: 5918164 → 5912876 bytes (OK)
```

All 29 tests pass (20 unit + 9 integration) plus 7 doc-tests. Clean build with zero warnings.

### PERFORMANCE BENCHMARKS (criterion, release mode)

**YouTube APK (7 DEX, 44MB):**

| Operation | Spec Target | Actual | vs Target |
|---|---|---|---|
| Parse single DEX (~8MB) | < 300ms | **73-75ms** | 4x faster |
| Parse skip verify | < 300ms | **72-74ms** | 4x faster |
| Parse all 7 DEX (44MB) | — | **431ms** | ~62ms/DEX |
| Write single DEX (~8MB) | < 200ms | **109ms** | 1.8x faster |
| Write all 7 DEX | — | **672ms** | ~96ms/DEX |
| Round-trip (parse+write) | < 500ms | **185ms** | 2.7x faster |
| Find method by name | < 50ms | **2.3µs** | 21,700x faster |
| Find all \<init\> methods | < 50ms | **0.49ms** | 102x faster |
| Pattern search (opcodes) | < 50ms | **5.3ms** | 10x faster |
| Find class | — | **4.4µs** | instant |
| Multi-dex parse (rayon) | — | **265ms** | 34% faster than sequential |
| Multi-dex write (rayon) | — | **307ms** | 26% faster than sequential |
| Multi-dex cross-search | — | **338µs** | fast |

**Instagram APK (17 DEX, 124MB):**

| Operation | Result |
|---|---|
| Parse largest DEX (10MB) | **57ms** |
| Parse all 17 DEX (sequential) | **736ms** (~43ms/DEX) |
| Write largest DEX | **84ms** |
| Round-trip largest | **141ms** |
| Multi-dex parse (rayon, 17 files) | **379ms** (49% faster than sequential) |
| Multi-dex write (rayon, 17 files) | **393ms** |
| Lazy parse largest (headers+IDs only) | **27ms** (2.1x faster than full parse) |

Note: Write times increased from Session 3 (~57ms) to Session 5 (~109ms) due to `sort_for_write()` cloning the DexFile (including raw buffer). The sort itself is needed for correctness; the clone overhead comes from `Arc<[u8]>` ref-counting. Actual serialization speed is unchanged.

Mutation benchmarks (return_early, intern_method, intern_string) measure ~12ms each, but this is dominated by DexFile::clone() overhead in criterion's iter_batched setup. Actual mutation operations are sub-microsecond.

### WRITER BUGS FIXED (Session 1)

1. **class_data code_off=0 bug (CRITICAL)**: class_data was written BEFORE code items, so code_off was always 0. LEB128 is variable-length and can't be patched in place. FIX: restructured write_dex() to write code items + debug info FIRST, then pass collected offsets to write_class_data().

2. **Try item handler_off wrong**: Writer was using handler_idx (array index) as handler_off, but DEX format needs byte offset relative to handler list start. FIX: build catch handler data in temp buffer, compute byte offsets, write try items with correct offsets.

3. **Annotation set ref list interleaving**: Parameter annotations wrote size, then annotation items/sets interleaved, then offsets — but format requires size + offsets contiguous. FIX: write annotation sets first, collect offsets, then write ref list (size + offsets together).

4. **Debug info bloat (5MB → 62KB)**: Original DEX shares debug_info entries across methods with identical debug state. Our reader duplicated them. FIX: dedup debug info by serializing to temp buffer and caching by content. Only 1,269 of 36,755 debug infos were unique in classes.dex.

5. **Annotation item/set dedup**: Added caching for annotation items (by binary content) and annotation sets (by item offset list).

### BUGS/IMPROVEMENTS FIXED (Session 2) — 16 of 18

**Correctness:**
1. Handler lookup: `unwrap_or(0)` → proper `ok_or(DexError::InvalidOffset)` error
2. `find_method`/`find_method_mut`: was using MethodIdx as string index; now resolves through method table
3. TypeIdx: u16 → u32 to prevent overflow with >65535 types (changed across ~15 files)
4. `code_units()`: fixed formulas for PackedSwitch, SparseSwitch, FillArrayData payloads
5. Handler byte offset: clarified absolute position calculation with clear `pos` variable

**Anti-Rust patterns:**
7. Writer: extracted `methods_with_code()` helper, replaced 3 nested class→method→code loops
8. Map entries: `clone()` → `std::mem::take()` to avoid allocation
9. String data offsets patching: eliminated take+reassign hack

**Maintainability:**
11. Instruction enum: added `Eq` derive
12. Writer: replaced magic hex offsets with named constants
13. `is_default_value`: fixed Float(-0.0)/Double(-0.0) using `to_bits()` for IEEE 754 correctness
14. `insert_instructions`: O(n²) repeated insert → O(n) `Vec::splice`
15. Removed unused `indexmap` dependency from Cargo.toml
16. Added `#[non_exhaustive]` to EncodedValue, DexError, Instruction enums
17. SparseSwitchPayload: separate keys/targets vecs → `keys_and_targets: Vec<(i32, i32)>`
18. Bounds checking: `u16_at`/`u32_at`/`i32_at` now use `buf.get()` with explicit panic messages

**Not applicable / already clean:**
6. No `eprintln!` calls found — already clean
10. Pub field encapsulation — maps already private, further encapsulation deferred

### NEW FEATURES (Session 3)

1. **Hidden API reader/writer**: Parses and serializes `TYPE_HIDDENAPI_CLASS_DATA_ITEM` (0xF000) for DEX 039+.

2. **Mutation API**: `intern_method(class, name, proto)`, `intern_field(class, name, type_)`, `intern_proto(descriptor)` — full reference interning with dedup.

3. **Multi-DEX support** (`MultiDexContainer`): parse/write multiple DEX files, cross-DEX class lookup, APK loading (requires `zip` feature).

4. **Fingerprint search API**: `find_method_by(predicate)`, `find_methods_by(predicate)`, `find_methods_with_opcodes(pattern)` with `MethodMatch`, `InstructionPattern`, `OpcodeMatcher`.

5. **Map list annotation counts fixed**: `TYPE_ANNOTATION_SET_ITEM`, `TYPE_ANNOTATION_SET_REF_LIST`, `TYPE_ANNOTATIONS_DIRECTORY_ITEM` now properly tracked.

6. **DexFile derives Clone** for benchmarking and general use.

7. **Criterion benchmarks**: Full benchmark suite in `benches/dex_bench.rs` covering parse, write, round-trip, search, mutation, and multi-dex.

### All source files:

**Cargo.toml**: thiserror 2, bitflags 2, smallvec 1, sha1 0.10, adler 1. Optional: zip 2, memmap2 0.9, rayon 1.10. Dev: pretty_assertions 1, zip 2, criterion 0.5. Default features: mmap, parallel.

**src/error.rs**: DexError enum, Result type alias

**src/encoding/**:
- leb128.rs: read/write ULEB128, SLEB128, ULEB128p1 (tested)
- mutf8.rs: decode/encode MUTF-8 with surrogate pairs (tested)
- encoded_value.rs: write_encoded_value/array/annotation for all 18 value types

**src/model/** (all files):
- access_flags.rs, header.rs, string.rs, types.rs, proto.rs, field.rs, method.rs
- class.rs (ClassDef, ClassData, EncodedField/Method, mutation helpers)
- code.rs (CodeItem with return_early, insert/remove instructions)
- instruction.rs (full ~200 variant Instruction enum)
- debug.rs, annotation.rs, encoded_value.rs, call_site.rs, method_handle.rs
- hidden_api.rs, map.rs
- dex_file.rs (DexFile + intern/find/mutation/search API + MethodMatch + InstructionPattern + OpcodeMatcher)

**src/reader/** (all files, WORKING):
- header_reader.rs, id_reader.rs, class_reader.rs, code_reader.rs
- debug_reader.rs, annotation_reader.rs, encoded_value_reader.rs, parse.rs (incl. hidden API reader)

**src/writer/** (all files, WORKING):
- code_writer.rs (instruction encoder), debug_writer.rs, write.rs (main writer incl. hidden API writer)
- sort.rs (table sorting + full index remapping for write correctness)

**src/multi_dex.rs**: MultiDexContainer

**src/util/**: sort.rs (dex_string_compare), descriptor.rs (parse_method_descriptor)

**src/lib.rs**: re-exports all public types

**tests/round_trip.rs**: 8 integration tests (parse, round-trip, multi-dex, intern, fingerprint search, mutation, raw buffer, lazy parsing)

**benches/dex_bench.rs**: criterion benchmarks (parse, write, round-trip, search, mutation, multi-dex, instagram)

**fuzz/fuzz_targets/**: 5 fuzz targets (fuzz_parse, fuzz_parse_lenient, fuzz_leb128, fuzz_round_trip, fuzz_mutf8)

### SPEC COMPLETION (Session 4)

9. **Writer table sorting**: `sort_for_write()` in `src/writer/sort.rs` — sorts strings (UTF-16 order), types (by descriptor_idx), protos (by return_type then params), fields (by class, name, type), methods (by class, name, proto) and remaps ALL index references throughout the entire DexFile (instructions, annotations, debug info, encoded values, call sites, method handles, class data, etc.). Fast-path: detects identity permutations (already sorted) and skips the clone.

10. **Branch offset auto-fixup**: `insert_instruction()`, `insert_instructions()`, and `remove_instruction()` now automatically adjust all branch offsets (goto, if-*, switch, fill-array-data), try item start_addr/insn_count, and catch handler addresses.

11. **DexFile derives Clone** (moved here from session 3 notes).

### PERFORMANCE & INFRASTRUCTURE (Session 5)

12. **Zero-copy / raw buffer retention**: `DexFile.raw: Option<Arc<[u8]>>` stores the original buffer during parse. Accessible via `dex.raw_buffer()`. Enables lazy resolution and zero-copy access to original data.

13. **Lazy parsing mode**: `ParseOptions { lazy: true, .. }` skips class_data and code item parsing upfront. Class data offsets stored in `lazy_class_data_offsets`. Resolve on demand via `dex.resolve_class_data(idx)` or `dex.resolve_all_class_data()`. Useful for tools that only inspect a few classes.

14. **Memory-mapped I/O** (`mmap` feature, default on): `dex_rs::parse_file(path, opts)` uses `memmap2` to memory-map the file, avoiding a full read into heap. Falls back to `std::fs::read` when feature is disabled.

15. **Parallel parsing** (`parallel` feature, default on): `MultiDexContainer::parse()` and `write_all()` use `rayon` to parse/write DEX files in parallel. Falls back to sequential when feature is disabled.

16. **Fuzz testing setup**: 5 fuzz targets in `fuzz/fuzz_targets/`:
    - `fuzz_parse` — strict parse of arbitrary bytes (must not panic)
    - `fuzz_parse_lenient` — lenient parse of arbitrary bytes
    - `fuzz_leb128` — ULEB128/SLEB128/ULEB128p1 decoder
    - `fuzz_round_trip` — parse → write → re-parse (if parse succeeds, write must not panic)
    - `fuzz_mutf8` — MUTF-8 decode/encode round-trip
    Run with: `cargo +nightly fuzz run <target>` (requires nightly + C++ toolchain)

### Feature flags (Cargo.toml):
```toml
[features]
default = ["mmap", "parallel"]
mmap = ["dep:memmap2"]       # Memory-mapped file I/O
parallel = ["dep:rayon"]     # Parallel multi-dex parsing/writing
zip = ["dep:zip"]            # APK/ZIP support
```

All 29 tests pass (20 unit + 9 integration) plus 7 doc-tests. Clean build with zero warnings.

### MAINTAINABILITY / API HARDENING / DOCS (Session 6)

17. **API hardening**:
    - `DexFile::resolve_class_data()` now returns `DexError::IndexOutOfBounds` for bad class indexes instead of panicking.
    - `intern_proto()`, `intern_method()`, and `intern_field()` now validate descriptors and return `DexError::InvalidDescriptor`.
    - Optional section parsing is stricter: invalid call sites and hidden API flags now return structured errors instead of being silently dropped or defaulted.
    - Parser panic capture added via `DexError::ParserPanic` for truncated/internal offset failures reached through the parser path.

18. **Structural refactor of large modules**:
    - `src/model/dex_file.rs` split into `src/model/dex_file/`:
      - `mod.rs`, `interning.rs`, `search.rs`, `version.rs`, `pattern.rs`, `tests.rs`
    - `src/reader/code_reader.rs` split into `src/reader/code_reader/`:
      - `mod.rs`, `orchestration.rs`, `decode.rs`, `format.rs`, `payload.rs`, `invoke.rs`, `arithmetic.rs`, `memory.rs`
    - `src/writer/write.rs` split into `src/writer/write/`:
      - `mod.rs`, `orchestration.rs`, `types.rs`, `code.rs`, `classdata.rs`, `annotations.rs`, `finalize.rs`

19. **Manual cleanup after refactor**:
    - Removed crate-level `#![allow(...)]` lint suppression that was temporarily introduced during refactoring.
    - Re-reviewed split reader/writer/model modules and fixed leftover issues (unused imports, dead helpers, unreachable match arm, `NO_INDEX` resolution, overcomplicated finalize plumbing).
    - Comment pass completed: removed low-value narration comments and kept only invariant/intent-focused Rust-style docs where helpful.

20. **Crate/module documentation**:
    - Added crate docs in `src/lib.rs`.
    - Added top-level docs for `src/model/mod.rs`, `src/reader/mod.rs`, `src/writer/mod.rs`, `src/error.rs`, `src/model/header.rs`, and tightened docs in `src/multi_dex.rs`.
    - Added rustdoc usage examples for:
      - crate-level parse/write flow
      - `parse_file()`
      - `write()`
      - `MultiDexContainer::parse()`
      - `MultiDexContainer::write_all()`
      - `DexFile::find_methods_with_opcodes()`

21. **Verification status after refactor + docs**:
    - `cargo fmt --all` passes
    - `cargo clippy --all-targets --all-features -- -D warnings` passes
    - `cargo test` passes
    - doc-tests: 7 passed

22. **Behavior check after refactor**:
    - Successful-path behavior remains intact for parse/search/lazy-parse/multi-dex/round-trip flows.
    - Current APK-backed integration tests still parse and round-trip the fixture APKs successfully.
    - Semantic behavior changes are intentional hardening changes (structured errors instead of panic/silent fallback on invalid inputs).

### CURRENT SOURCE LAYOUT (updated)

**src/model/**:
- Core leaf types remain in individual files (`access_flags.rs`, `annotation.rs`, `call_site.rs`, `class.rs`, `code.rs`, `debug.rs`, `encoded_value.rs`, `field.rs`, `header.rs`, `hidden_api.rs`, `instruction.rs`, `map.rs`, `method.rs`, `method_handle.rs`, `proto.rs`, `string.rs`, `types.rs`)
- `dex_file/` now owns the higher-level DexFile API surface (`mod.rs`, `interning.rs`, `search.rs`, `version.rs`, `pattern.rs`, `tests.rs`)

**src/reader/**:
- Existing readers: `header_reader.rs`, `id_reader.rs`, `class_reader.rs`, `debug_reader.rs`, `annotation_reader.rs`, `encoded_value_reader.rs`, `parse.rs`
- `code_reader/` now contains the split code-item decoder (`mod.rs`, `orchestration.rs`, `decode.rs`, `format.rs`, `payload.rs`, `invoke.rs`, `arithmetic.rs`, `memory.rs`)

**src/writer/**:
- Existing helpers: `code_writer.rs`, `debug_writer.rs`, `sort.rs`
- `write/` now contains the split main serializer (`mod.rs`, `orchestration.rs`, `types.rs`, `code.rs`, `classdata.rs`, `annotations.rs`, `finalize.rs`)

**Public entrypoints**:
- `src/lib.rs`: crate docs + public re-exports
- `src/reader/mod.rs`: parse entrypoints (`parse`, `parse_file`)
- `src/writer/mod.rs`: write entrypoint (`write`)
- `src/multi_dex.rs`: `MultiDexContainer`

### REMAINING ITEMS
- Bumpalo arena allocation (requires lifetime params on IR — invasive refactor, marginal gain)
- Cow<[u8]> for string data (strings are MUTF-8 decoded, so already copied)
- SmallVec for invoke arg lists

### KEY SPEC REMINDERS FOR FUTURE SELF
- class_data fields/methods are DELTA-encoded via ULEB128
- code_off in class_data is ULEB128 — variable length, CANNOT patch in place
- Code items need 4-byte alignment
- Header checksum = Adler32([12..file_size]), signature = SHA1([32..file_size])
- String sort = UTF-16 code unit order
- Try item handler_off = byte offset relative to handler list start (not an index!)
- Annotation set ref list for params: size(u32) then offset(u32) per parameter, offsets must be contiguous
- Debug info entries are heavily shared in real DEX files — always dedup
