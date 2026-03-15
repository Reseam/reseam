# `dex-rs`: Complete Specification for a Production-Grade DEX Parser/Writer in Rust

**Version:** 1.0
**Target parity:** dexlib2 (smali/baksmali project)
**Scope:** Parse, represent, mutate, and write DEX files with full round-trip fidelity

---

## Table of Contents

1. [Goals and Non-Goals](#1-goals-and-non-goals)
2. [DEX File Format Overview](#2-dex-file-format-overview)
3. [Binary Encoding Primitives](#3-binary-encoding-primitives)
4. [Header Section](#4-header-section)
5. [String ID and Data Sections](#5-string-id-and-data-sections)
6. [Type IDs](#6-type-ids)
7. [Prototype IDs](#7-prototype-ids)
8. [Field IDs](#8-field-ids)
9. [Method IDs](#9-method-ids)
10. [Class Definitions](#10-class-definitions)
11. [Class Data](#11-class-data)
12. [Code Items (Method Bodies)](#12-code-items-method-bodies)
13. [Instruction Set Architecture](#13-instruction-set-architecture)
14. [Debug Info](#14-debug-info)
15. [Annotations](#15-annotations)
16. [Map List](#16-map-list)
17. [Encoded Values](#17-encoded-values)
18. [Call Site and Method Handle Sections (DEX 038+)](#18-call-site-and-method-handle-sections-dex-038)
19. [HiddenAPI Data (DEX 039+)](#19-hiddenapi-data-dex-039)
20. [In-Memory IR Design](#20-in-memory-ir-design)
21. [Writer / Serializer Design](#21-writer--serializer-design)
22. [Round-Trip Fidelity Requirements](#22-round-trip-fidelity-requirements)
23. [Mutation API](#23-mutation-api)
24. [Error Handling Strategy](#24-error-handling-strategy)
25. [Performance Requirements](#25-performance-requirements)
26. [Testing Strategy](#26-testing-strategy)
27. [Crate Structure and Public API Surface](#27-crate-structure-and-public-api-surface)
28. [Known Edge Cases and Pitfalls](#28-known-edge-cases-and-pitfalls)
29. [CDEX / VDEX Compact DEX (Future)](#29-cdex--vdex-compact-dex-future)
30. [Appendix A: Complete Opcode Table](#appendix-a-complete-opcode-table)
31. [Appendix B: Type Descriptor Grammar](#appendix-b-type-descriptor-grammar)
32. [Appendix C: Access Flag Definitions](#appendix-c-access-flag-definitions)
33. [Appendix D: Annotation Visibility Constants](#appendix-d-annotation-visibility-constants)
34. [Appendix E: Reference Comparison with dexlib2](#appendix-e-reference-comparison-with-dexlib2)

---

## 1. Goals and Non-Goals

### Goals

- **Full round-trip fidelity**: `parse(bytes) |> write() == bytes` for unmodified DEX files. Byte-identical output when no mutations are applied.
- **Feature parity with dexlib2**: Support every DEX version (035, 037, 038, 039), every instruction format, every metadata section, annotations, debug info, call sites, method handles, and hidden API flags.
- **Zero-copy parsing where possible**: String data, instruction bytes, and debug info should reference the original buffer via slices or `Cow<'a, [u8]>` when the IR has not been mutated.
- **Safe, ergonomic mutation API**: Typed instruction builders, method/class/field insertion and removal, automatic index/offset recalculation on write.
- **Performance**: Parse a 15MB DEX file (YouTube-scale `classes.dex`) in under 200ms on modern hardware. Write in under 300ms.
- **Strict conformance**: Reject malformed DEX files with clear error messages. Follow the AOSP DEX specification exactly.
- **No unsafe in public API**: All `unsafe` blocks are internal, documented, and auditable.

### Non-Goals

- Smali text assembly/disassembly (separate crate if desired).
- DEX optimization (dex2oat, OAT files, ART profiles).
- Multi-DEX container management (that belongs in the APK-level crate).
- Java/Kotlin class file parsing.
- CDEX/VDEX in initial version (spec provided for future work in Section 29).

---

## 2. DEX File Format Overview

A DEX file is a single contiguous byte sequence with the following layout:

```
+----------------------------+  offset 0x00
|        Header              |  112 bytes (fixed size)
+----------------------------+
|      String IDs            |  4 bytes each (offset into string_data)
+----------------------------+
|       Type IDs             |  4 bytes each (index into string_ids)
+----------------------------+
|     Prototype IDs          |  12 bytes each
+----------------------------+
|      Field IDs             |  8 bytes each
+----------------------------+
|     Method IDs             |  8 bytes each
+----------------------------+
|    Class Definitions       |  32 bytes each
+----------------------------+
|        Call Site IDs       |  4 bytes each (DEX 038+)
+----------------------------+
|      Method Handles        |  8 bytes each (DEX 038+)
+----------------------------+
|         Data Region        |  Variable: string data, class data,
|                            |  code items, debug info, annotations,
|                            |  type lists, encoded arrays, map list
+----------------------------+
|         Link Data          |  Typically empty (reserved for static linking)
+----------------------------+
```

**Byte order**: Always little-endian.
**Alignment**: The file is structured with specific alignment requirements per section (detailed in each section below).

### DEX Versions

| Magic bytes (first 8) | Version | New features |
|---|---|---|
| `dex\n035\0` | 035 | Base DEX format |
| `dex\n037\0` | 037 | Default methods in interfaces |
| `dex\n038\0` | 038 | Invoke-polymorphic, invoke-custom, call sites, method handles |
| `dex\n039\0` | 039 | Hidden API restrictions metadata |

The parser MUST accept all four versions. The writer MUST emit the minimum version required by the content (e.g., if no call sites or method handles exist, write 035 or 037).

---

## 3. Binary Encoding Primitives

### LEB128 (Little-Endian Base 128)

Used extensively throughout the data region.

**Unsigned LEB128 (ULEB128)**:
- Each byte contributes 7 bits of payload. High bit indicates continuation (1 = more bytes follow, 0 = final byte).
- Maximum 5 bytes for 32-bit values.

```
value = 0
shift = 0
loop:
    byte = read_byte()
    value |= (byte & 0x7F) << shift
    if (byte & 0x80) == 0: break
    shift += 7
```

**Signed LEB128 (SLEB128)**:
- Same encoding, but sign-extended from the last byte.
- After loop, if `shift < 32` and the sign bit of the last byte is set, extend: `value |= -(1 << shift)`.

**ULEB128p1**:
- Read as ULEB128, then subtract 1. Used for nullable indices where 0 represents "no index" (maps to -1 / `NO_INDEX`).
- The value `0` encodes as ULEB128 value `1`, the value `-1` (no index) encodes as ULEB128 value `0`.

### Rust Implementation Requirements

```rust
/// Reads an unsigned LEB128 value from the buffer at the given position.
/// Returns (value, bytes_consumed).
/// Errors if more than 5 bytes are needed or the buffer is exhausted.
pub fn read_uleb128(buf: &[u8], pos: usize) -> Result<(u32, usize)>;

/// Reads a signed LEB128 value.
pub fn read_sleb128(buf: &[u8], pos: usize) -> Result<(i32, usize)>;

/// Reads ULEB128, subtracts 1. Returns Option<u32> where None = NO_INDEX.
pub fn read_uleb128p1(buf: &[u8], pos: usize) -> Result<(Option<u32>, usize)>;

/// Writes an unsigned LEB128 value. Returns bytes written.
pub fn write_uleb128(buf: &mut Vec<u8>, value: u32) -> usize;

/// Writes a signed LEB128 value.
pub fn write_sleb128(buf: &mut Vec<u8>, value: i32) -> usize;

/// Writes ULEB128p1. None encodes as ULEB128(0).
pub fn write_uleb128p1(buf: &mut Vec<u8>, value: Option<u32>) -> usize;
```

**CRITICAL edge case**: dexlib2 tolerates LEB128 values with unnecessary trailing zero bytes (over-long encodings). Some obfuscators produce these intentionally. The parser SHOULD accept them. The writer MUST produce canonical (minimal) encodings.

### MUTF-8 (Modified UTF-8)

Strings in DEX use MUTF-8, which differs from standard UTF-8:

- The null character U+0000 is encoded as `0xC0 0x80` (two bytes), never as `0x00`.
- Supplementary characters (U+10000 and above) are encoded as a surrogate pair, where each surrogate is encoded in 3-byte MUTF-8 form (6 bytes total), NOT as 4-byte UTF-8.
- Characters U+0001 through U+007F are encoded as single bytes (same as ASCII/UTF-8).
- Characters U+0080 through U+07FF use 2-byte encoding (same as UTF-8).
- Characters U+0800 through U+FFFF use 3-byte encoding (same as UTF-8).

```rust
/// Decodes a MUTF-8 string from raw bytes.
/// The input does NOT include the null terminator or length prefix.
pub fn decode_mutf8(bytes: &[u8]) -> Result<String>;

/// Encodes a Rust String to MUTF-8 bytes.
pub fn encode_mutf8(s: &str) -> Vec<u8>;
```

**Edge case**: dexlib2 leniently decodes some malformed MUTF-8 (e.g., lone surrogates). The parser should handle this without panicking, emitting a replacement character or preserving raw bytes.

---

## 4. Header Section

Fixed 112 bytes at offset 0.

| Offset | Size | Field | Description |
|---|---|---|---|
| 0x00 | 8 | `magic` | `dex\n0NN\0` where NN is the version |
| 0x08 | 4 | `checksum` | Adler-32 checksum of bytes [12..file_size] |
| 0x0C | 20 | `signature` | SHA-1 hash of bytes [32..file_size] |
| 0x20 | 4 | `file_size` | Total file size in bytes |
| 0x24 | 4 | `header_size` | Always 0x70 (112) |
| 0x28 | 4 | `endian_tag` | Always `0x12345678` (little-endian) |
| 0x2C | 4 | `link_size` | Size of link section (typically 0) |
| 0x30 | 4 | `link_off` | Offset to link section (0 if link_size == 0) |
| 0x34 | 4 | `map_off` | Offset to map list (MUST be non-zero) |
| 0x38 | 4 | `string_ids_size` | Count of string ID entries |
| 0x3C | 4 | `string_ids_off` | Offset to string IDs array |
| 0x40 | 4 | `type_ids_size` | Count of type ID entries (max 65535) |
| 0x44 | 4 | `type_ids_off` | Offset to type IDs array |
| 0x48 | 4 | `proto_ids_size` | Count of prototype ID entries (max 65535) |
| 0x4C | 4 | `proto_ids_off` | Offset to prototype IDs array |
| 0x50 | 4 | `field_ids_size` | Count of field ID entries |
| 0x54 | 4 | `field_ids_off` | Offset to field IDs array |
| 0x58 | 4 | `method_ids_size` | Count of method ID entries |
| 0x5C | 4 | `method_ids_off` | Offset to method IDs array |
| 0x60 | 4 | `class_defs_size` | Count of class definition entries |
| 0x64 | 4 | `class_defs_off` | Offset to class definitions array |
| 0x68 | 4 | `data_size` | Size of data section (must be 4-byte aligned) |
| 0x6C | 4 | `data_off` | Offset to start of data section |

### Rust Representation

```rust
#[derive(Debug, Clone)]
pub struct DexHeader {
    pub version: DexVersion,        // enum { V035, V037, V038, V039 }
    pub checksum: u32,
    pub signature: [u8; 20],
    pub file_size: u32,
    pub link_size: u32,
    pub link_off: u32,
    pub map_off: u32,
    pub string_ids_size: u32,
    pub string_ids_off: u32,
    pub type_ids_size: u32,
    pub type_ids_off: u32,
    pub proto_ids_size: u32,
    pub proto_ids_off: u32,
    pub field_ids_size: u32,
    pub field_ids_off: u32,
    pub method_ids_size: u32,
    pub method_ids_off: u32,
    pub class_defs_size: u32,
    pub class_defs_off: u32,
    pub data_size: u32,
    pub data_off: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DexVersion {
    V035,
    V037,
    V038,
    V039,
}
```

### Validation on Parse

- `magic` must match exactly one of the four known version strings.
- `header_size` must be 0x70.
- `endian_tag` must be 0x12345678.
- `file_size` must match actual buffer length.
- `checksum` must match Adler-32 of `buf[12..file_size]`.
- `signature` must match SHA-1 of `buf[32..file_size]`.
- All `*_off` values must be within `[0, file_size)`.
- `type_ids_size` and `proto_ids_size` must not exceed 65535.
- `data_size` must be 4-byte aligned.

**Lenient mode**: Optionally skip checksum/signature validation (needed for patching workflows where the file is being modified incrementally). Provide a `ParseOptions` struct:

```rust
#[derive(Debug, Clone)]
pub struct ParseOptions {
    /// Skip Adler-32 checksum verification.
    pub skip_checksum: bool,
    /// Skip SHA-1 signature verification.
    pub skip_signature: bool,
    /// Accept over-long LEB128 encodings.
    pub lenient_leb128: bool,
    /// Accept malformed MUTF-8 strings.
    pub lenient_mutf8: bool,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            skip_checksum: false,
            skip_signature: false,
            lenient_leb128: true,  // Enabled by default for compat
            lenient_mutf8: true,
        }
    }
}
```

### Writer Obligations

- Recompute `checksum` (Adler-32) and `signature` (SHA-1) after writing all other data.
- Set `file_size` to actual written length.
- Set `header_size` to 0x70.
- Set `endian_tag` to 0x12345678.
- Compute all `*_off` and `*_size` fields from actual written positions.

---

## 5. String ID and Data Sections

### String IDs Array

Located at `header.string_ids_off`. Contains `header.string_ids_size` entries, each 4 bytes:

| Offset | Size | Field | Description |
|---|---|---|---|
| 0x00 | 4 | `string_data_off` | Offset from start of file to string_data_item |

Entries MUST be sorted by string content (UTF-16 code unit ordering, not UTF-8 byte ordering). This is critical for binary search during resolution.

### String Data Items

Each `string_data_item` at the referenced offset:

| Component | Encoding | Description |
|---|---|---|
| `utf16_size` | ULEB128 | Number of UTF-16 code units (NOT byte length) |
| `data` | MUTF-8 | The string content, null-terminated |

The null terminator `0x00` follows the MUTF-8 data and is NOT included in `utf16_size`.

### Rust IR

```rust
/// Index into the string table. Newtype for type safety.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StringIdx(pub u32);

/// A resolved string. For unmodified DEX files, this can borrow
/// from the original buffer via Cow.
#[derive(Debug, Clone)]
pub struct DexString {
    /// The decoded UTF-8 string content.
    pub value: Cow<'static, str>,
    /// Original MUTF-8 byte length (for round-trip fidelity).
    /// None if this string was created new (not parsed).
    pub(crate) original_mutf8_len: Option<usize>,
}
```

### Sorting Invariant

When writing, strings must be sorted by UTF-16 code unit comparison (Java's `String.compareTo` semantics). This means:

1. Compare character by character using UTF-16 code unit values.
2. Supplementary characters (above U+FFFF) are compared as their surrogate pair code units.
3. This differs from Rust's default `str` ordering (which is UTF-8 byte order, and coincidentally produces the same result for BMP characters, but NOT for supplementary characters).

```rust
/// Compare two strings using DEX string sort order (UTF-16 code unit comparison).
pub fn dex_string_compare(a: &str, b: &str) -> std::cmp::Ordering {
    let a_units = a.encode_utf16();
    let b_units = b.encode_utf16();
    a_units.cmp(b_units)
}
```

---

## 6. Type IDs

Located at `header.type_ids_off`. Contains `header.type_ids_size` entries, each 4 bytes:

| Offset | Size | Field | Description |
|---|---|---|---|
| 0x00 | 4 | `descriptor_idx` | Index into `string_ids` for the type descriptor |

Type descriptors follow the JVM/Dalvik descriptor grammar (see Appendix B). Examples:
- `I` = int
- `Ljava/lang/String;` = java.lang.String
- `[B` = byte[]
- `[[Ljava/lang/Object;` = Object[][]

Entries MUST be sorted by `descriptor_idx` (i.e., by the string index of the descriptor). Maximum 65535 entries.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TypeIdx(pub u16);  // Note: u16, not u32. Max 65535.
```

---

## 7. Prototype IDs

Located at `header.proto_ids_off`. Contains `header.proto_ids_size` entries, each 12 bytes:

| Offset | Size | Field | Description |
|---|---|---|---|
| 0x00 | 4 | `shorty_idx` | Index into `string_ids` for the shorty descriptor |
| 0x04 | 4 | `return_type_idx` | Index into `type_ids` for the return type |
| 0x08 | 4 | `parameters_off` | Offset to `type_list` for parameters, or 0 if no params |

### Shorty Descriptors

A shorty descriptor is a condensed representation of the method signature:
- Return type first, then parameter types.
- `V` = void, `I` = int, `J` = long, `D` = double, `F` = float, `Z` = boolean, `B` = byte, `S` = short, `C` = char.
- `L` = any reference type (object or array). Arrays are also `L`, not `[`.

Example: A method `int foo(String s, int[] arr, double d)` has shorty `ILLI` ... wait no: return is `I`, params are `L` (String), `L` (int[]), `D` (double) = `ILLD`.

### Type Lists

A `type_list` item (referenced by `parameters_off` and also by class interfaces):

| Component | Size | Description |
|---|---|---|
| `size` | 4 | Number of entries |
| `list[size]` | 2 each | `type_idx` values (u16) |

**Alignment**: `type_list` must be 4-byte aligned.

Prototypes are sorted by: return type index first, then parameter list (lexicographic comparison of parameter type indices). Maximum 65535 entries.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProtoIdx(pub u16);

#[derive(Debug, Clone)]
pub struct Prototype {
    pub shorty: StringIdx,
    pub return_type: TypeIdx,
    pub parameters: Vec<TypeIdx>,  // Empty vec for no parameters
}
```

---

## 8. Field IDs

Located at `header.field_ids_off`. Contains `header.field_ids_size` entries, each 8 bytes:

| Offset | Size | Field | Description |
|---|---|---|---|
| 0x00 | 2 | `class_idx` | Index into `type_ids` for the defining class |
| 0x02 | 2 | `type_idx` | Index into `type_ids` for the field type |
| 0x04 | 4 | `name_idx` | Index into `string_ids` for the field name |

Sorted by: defining class (type_idx order), then field name (string_idx order), then field type (type_idx order).

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FieldIdx(pub u32);

#[derive(Debug, Clone)]
pub struct FieldId {
    pub class: TypeIdx,
    pub type_: TypeIdx,
    pub name: StringIdx,
}
```

---

## 9. Method IDs

Located at `header.method_ids_off`. Contains `header.method_ids_size` entries, each 8 bytes:

| Offset | Size | Field | Description |
|---|---|---|---|
| 0x00 | 2 | `class_idx` | Index into `type_ids` for the defining class |
| 0x02 | 2 | `proto_idx` | Index into `proto_ids` for the method prototype |
| 0x04 | 4 | `name_idx` | Index into `string_ids` for the method name |

Sorted by: defining class, then method name, then prototype.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MethodIdx(pub u32);

#[derive(Debug, Clone)]
pub struct MethodId {
    pub class: TypeIdx,
    pub proto: ProtoIdx,
    pub name: StringIdx,
}
```

---

## 10. Class Definitions

Located at `header.class_defs_off`. Contains `header.class_defs_size` entries, each 32 bytes:

| Offset | Size | Field | Description |
|---|---|---|---|
| 0x00 | 4 | `class_idx` | Index into `type_ids` for this class |
| 0x04 | 4 | `access_flags` | Access flags (see Appendix C) |
| 0x08 | 4 | `superclass_idx` | Index into `type_ids` for superclass, or `NO_INDEX` |
| 0x0C | 4 | `interfaces_off` | Offset to `type_list` of interfaces, or 0 |
| 0x10 | 4 | `source_file_idx` | Index into `string_ids` for source file name, or `NO_INDEX` |
| 0x14 | 4 | `annotations_off` | Offset to `annotations_directory_item`, or 0 |
| 0x18 | 4 | `class_data_off` | Offset to `class_data_item`, or 0 |
| 0x1C | 4 | `static_values_off` | Offset to `encoded_array_item` for static field initial values, or 0 |

`NO_INDEX` = `0xFFFFFFFF`

Classes must be ordered such that a class's superclass and implemented interfaces appear earlier in the array than the class itself. This is the "topological sort" requirement.

```rust
#[derive(Debug, Clone)]
pub struct ClassDef {
    pub class_type: TypeIdx,
    pub access_flags: AccessFlags,
    pub superclass: Option<TypeIdx>,
    pub interfaces: Vec<TypeIdx>,
    pub source_file: Option<StringIdx>,
    pub annotations: Option<AnnotationsDirectory>,
    pub class_data: Option<ClassData>,
    pub static_values: Vec<EncodedValue>,
}
```

---

## 11. Class Data

Referenced by `class_def.class_data_off`. Entirely LEB128-encoded.

### Encoding

```
static_fields_size    : ULEB128
instance_fields_size  : ULEB128
direct_methods_size   : ULEB128
virtual_methods_size  : ULEB128
static_fields[static_fields_size]    : encoded_field
instance_fields[instance_fields_size]: encoded_field
direct_methods[direct_methods_size]  : encoded_method
virtual_methods[virtual_methods_size]: encoded_method
```

### Encoded Field

```
field_idx_diff : ULEB128   // Difference from previous field index (first is absolute)
access_flags   : ULEB128
```

### Encoded Method

```
method_idx_diff : ULEB128  // Difference from previous method index (first is absolute)
access_flags    : ULEB128
code_off        : ULEB128  // Offset to code_item, or 0 for abstract/native
```

**CRITICAL**: The `_diff` encoding means fields/methods are delta-encoded. The first entry is an absolute index. Subsequent entries store the difference from the previous index. When writing, you must sort fields and methods by their index and compute deltas.

```rust
#[derive(Debug, Clone)]
pub struct ClassData {
    pub static_fields: Vec<EncodedField>,
    pub instance_fields: Vec<EncodedField>,
    pub direct_methods: Vec<EncodedMethod>,
    pub virtual_methods: Vec<EncodedMethod>,
}

#[derive(Debug, Clone)]
pub struct EncodedField {
    pub field: FieldIdx,        // Resolved absolute index
    pub access_flags: AccessFlags,
}

#[derive(Debug, Clone)]
pub struct EncodedMethod {
    pub method: MethodIdx,      // Resolved absolute index
    pub access_flags: AccessFlags,
    pub code: Option<CodeItem>, // None if abstract/native
}
```

### Direct vs Virtual Methods

- **Direct methods**: `static`, `private`, or constructors (`<init>`, `<clinit>`). Not subject to virtual dispatch.
- **Virtual methods**: Everything else. Subject to virtual dispatch and overriding.

---

## 12. Code Items (Method Bodies)

Referenced by `encoded_method.code_off`. **4-byte aligned**.

### Fixed Header (16 bytes)

| Offset | Size | Field | Description |
|---|---|---|---|
| 0x00 | 2 | `registers_size` | Number of registers used |
| 0x02 | 2 | `ins_size` | Number of incoming argument words |
| 0x04 | 2 | `outs_size` | Number of outgoing argument words |
| 0x06 | 2 | `tries_size` | Number of try_item entries |
| 0x08 | 4 | `debug_info_off` | Offset to debug_info_item, or 0 |
| 0x0C | 4 | `insns_size` | Number of 16-bit code units in instructions array |

### Instructions

Immediately following the header: `insns_size` 16-bit code units. Dalvik instructions are 16-bit aligned and vary from 1 to 5 code units in length.

### Padding

If `tries_size > 0` and `insns_size` is odd, 2 bytes of zero padding follow the instructions to achieve 4-byte alignment for the tries array.

### Try Items

If `tries_size > 0`, immediately after instructions (and padding):

```
try_item[tries_size]:
    start_addr  : u32   // Start address (code unit offset into insns)
    insn_count  : u16   // Number of code units covered
    handler_off : u16   // Offset in bytes from start of encoded_catch_handler_list
```

Try items MUST be sorted by `start_addr` and MUST NOT overlap.

### Catch Handler List

Immediately after try items:

```
encoded_catch_handler_list:
    size        : ULEB128  // Number of handler entries
    list[size]  : encoded_catch_handler
```

Each `encoded_catch_handler`:

```
    size           : SLEB128  // Number of typed catches. If <= 0, there's a catch-all.
    handlers[abs(size)]:
        type_idx   : ULEB128  // Index into type_ids
        addr       : ULEB128  // Handler code unit address
    catch_all_addr : ULEB128  // Only present if size <= 0
```

If `size` is negative, `abs(size)` typed handlers are present, followed by a catch-all address. If `size` is zero, only a catch-all is present.

```rust
#[derive(Debug, Clone)]
pub struct CodeItem {
    pub registers_size: u16,
    pub ins_size: u16,
    pub outs_size: u16,
    pub debug_info: Option<DebugInfo>,
    pub instructions: Vec<Instruction>,
    pub tries: Vec<TryItem>,
    pub catch_handlers: Vec<CatchHandler>,
}

#[derive(Debug, Clone)]
pub struct TryItem {
    pub start_addr: u32,
    pub insn_count: u16,
    pub handler: CatchHandlerRef, // Index into catch_handlers
}

#[derive(Debug, Clone)]
pub struct CatchHandler {
    pub typed_catches: Vec<TypedCatch>,
    pub catch_all_addr: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct TypedCatch {
    pub exception_type: TypeIdx,
    pub addr: u32,
}
```

---

## 13. Instruction Set Architecture

Dalvik instructions use a fixed set of formats. Each instruction is identified by a 1-byte opcode (the low 8 bits of the first code unit).

### Instruction Formats

| Format | Size (units) | Layout | Description |
|---|---|---|---|
| 10x | 1 | `ØØ|op` | No operands (e.g., nop, return-void) |
| 12x | 1 | `B|A|op` | Two 4-bit register operands |
| 11n | 1 | `B|A|op` | One 4-bit reg + one 4-bit signed literal |
| 11x | 1 | `AA|op` | One 8-bit register operand |
| 10t | 1 | `AA|op` | One 8-bit signed branch offset |
| 20t | 2 | `ØØ|op AAAA` | One 16-bit signed branch offset |
| 22x | 2 | `AA|op BBBB` | Two registers (8-bit + 16-bit) |
| 21t | 2 | `AA|op BBBB` | One 8-bit reg + 16-bit signed branch offset |
| 21s | 2 | `AA|op BBBB` | One 8-bit reg + 16-bit signed literal |
| 21h | 2 | `AA|op BBBB` | One 8-bit reg + 16-bit high literal (shifted left 16 or 48) |
| 21c | 2 | `AA|op BBBB` | One 8-bit reg + 16-bit index (string/type/field/method) |
| 23x | 2 | `AA|op CC|BB` | Three 8-bit register operands |
| 22b | 2 | `AA|op CC|BB` | Two registers + 8-bit signed literal |
| 22t | 2 | `A|B|op CCCC` | Two 4-bit regs + 16-bit signed branch offset |
| 22s | 2 | `A|B|op CCCC` | Two 4-bit regs + 16-bit signed literal |
| 22c | 2 | `A|B|op CCCC` | Two 4-bit regs + 16-bit index |
| 30t | 3 | `ØØ|op AAAA_lo AAAA_hi` | One 32-bit signed branch offset |
| 32x | 3 | `ØØ|op AAAA BBBB` | Two 16-bit register operands |
| 31i | 3 | `AA|op BBBB_lo BBBB_hi` | One 8-bit reg + 32-bit literal |
| 31t | 3 | `AA|op BBBB_lo BBBB_hi` | One 8-bit reg + 32-bit offset (for switch) |
| 31c | 3 | `AA|op BBBB_lo BBBB_hi` | One 8-bit reg + 32-bit string index (jumbo) |
| 35c | 3 | `A|G|op BBBB F|E|D|C` | Invoke: 4-bit count + 4-bit regs + 16-bit method |
| 3rc | 3 | `AA|op BBBB CCCC` | Invoke/range: count + method + first register |
| 45cc | 4 | `A|G|op BBBB F|E|D|C HHHH` | Invoke-polymorphic (DEX 038+) |
| 4rcc | 4 | `AA|op BBBB CCCC HHHH` | Invoke-polymorphic/range (DEX 038+) |
| 51l | 5 | `AA|op BBBB_0 BBBB_1 BBBB_2 BBBB_3` | One 8-bit reg + 64-bit literal |

### Payload Formats (embedded in instruction stream)

**Packed-switch payload** (at 4-byte aligned offset from method start):
```
ident           : u16  // Always 0x0100
size            : u16  // Number of entries
first_key       : i32  // First (lowest) switch case value
targets[size]   : i32  // Branch targets (relative to switch instruction)
```

**Sparse-switch payload**:
```
ident           : u16  // Always 0x0200
size            : u16  // Number of entries
keys[size]      : i32  // Sorted switch case values
targets[size]   : i32  // Corresponding branch targets
```

**Fill-array-data payload**:
```
ident           : u16  // Always 0x0300
element_width   : u16  // Bytes per element (1, 2, 4, 8)
size            : u32  // Number of elements
data[size * element_width] : u8  // Element data
```
Followed by 2 bytes of padding if `size * element_width` is odd (for 2-byte alignment).

### Rust Instruction Representation

This is the core of the IR. Each instruction should be a typed enum:

```rust
/// A decoded Dalvik instruction.
#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    Nop,
    Move { dest: u4, src: u4 },
    MoveFrom16 { dest: u8, src: u16 },
    Move16 { dest: u16, src: u16 },
    MoveWide { dest: u4, src: u4 },
    MoveWideFrom16 { dest: u8, src: u16 },
    MoveWide16 { dest: u16, src: u16 },
    MoveObject { dest: u4, src: u4 },
    MoveObjectFrom16 { dest: u8, src: u16 },
    MoveObject16 { dest: u16, src: u16 },
    MoveResult { dest: u8 },
    MoveResultWide { dest: u8 },
    MoveResultObject { dest: u8 },
    MoveException { dest: u8 },
    ReturnVoid,
    Return { src: u8 },
    ReturnWide { src: u8 },
    ReturnObject { src: u8 },

    Const4 { dest: u4, value: i4 },
    Const16 { dest: u8, value: i16 },
    Const { dest: u8, value: i32 },
    ConstHigh16 { dest: u8, value: i16 },  // value << 16
    ConstWide16 { dest: u8, value: i16 },
    ConstWide32 { dest: u8, value: i32 },
    ConstWide { dest: u8, value: i64 },
    ConstWideHigh16 { dest: u8, value: i16 },  // value << 48
    ConstString { dest: u8, string: StringIdx },
    ConstStringJumbo { dest: u8, string: StringIdx },  // 32-bit index
    ConstClass { dest: u8, type_: TypeIdx },
    ConstMethodHandle { dest: u8, method_handle: MethodHandleIdx },
    ConstMethodType { dest: u8, proto: ProtoIdx },

    // Monitor
    MonitorEnter { ref_: u8 },
    MonitorExit { ref_: u8 },

    // Type checks
    CheckCast { ref_: u8, type_: TypeIdx },
    InstanceOf { dest: u4, ref_: u4, type_: TypeIdx },
    ArrayLength { dest: u4, array: u4 },

    // Object/array creation
    NewInstance { dest: u8, type_: TypeIdx },
    NewArray { dest: u4, size: u4, type_: TypeIdx },
    FilledNewArray { type_: TypeIdx, args: SmallVec<[u4; 5]> },
    FilledNewArrayRange { type_: TypeIdx, first_reg: u16, count: u8 },
    FillArrayData { array: u8, payload_offset: i32 },

    // Throw
    Throw { exception: u8 },

    // Branches
    Goto { offset: i8 },
    Goto16 { offset: i16 },
    Goto32 { offset: i32 },
    PackedSwitch { test: u8, payload_offset: i32 },
    SparseSwitch { test: u8, payload_offset: i32 },

    // Comparisons
    CmpLFloat { dest: u8, a: u8, b: u8 },   // cmpl-float
    CmpGFloat { dest: u8, a: u8, b: u8 },   // cmpg-float
    CmpLDouble { dest: u8, a: u8, b: u8 },  // cmpl-double
    CmpGDouble { dest: u8, a: u8, b: u8 },  // cmpg-double
    CmpLong { dest: u8, a: u8, b: u8 },

    // If-test (two registers)
    IfEq { a: u4, b: u4, offset: i16 },
    IfNe { a: u4, b: u4, offset: i16 },
    IfLt { a: u4, b: u4, offset: i16 },
    IfGe { a: u4, b: u4, offset: i16 },
    IfGt { a: u4, b: u4, offset: i16 },
    IfLe { a: u4, b: u4, offset: i16 },

    // If-testz (one register vs zero)
    IfEqz { a: u8, offset: i16 },
    IfNez { a: u8, offset: i16 },
    IfLtz { a: u8, offset: i16 },
    IfGez { a: u8, offset: i16 },
    IfGtz { a: u8, offset: i16 },
    IfLez { a: u8, offset: i16 },

    // Array operations (all format 23x)
    Aget { dest: u8, array: u8, index: u8 },
    AgetWide { dest: u8, array: u8, index: u8 },
    AgetObject { dest: u8, array: u8, index: u8 },
    AgetBoolean { dest: u8, array: u8, index: u8 },
    AgetByte { dest: u8, array: u8, index: u8 },
    AgetChar { dest: u8, array: u8, index: u8 },
    AgetShort { dest: u8, array: u8, index: u8 },
    Aput { src: u8, array: u8, index: u8 },
    AputWide { src: u8, array: u8, index: u8 },
    AputObject { src: u8, array: u8, index: u8 },
    AputBoolean { src: u8, array: u8, index: u8 },
    AputByte { src: u8, array: u8, index: u8 },
    AputChar { src: u8, array: u8, index: u8 },
    AputShort { src: u8, array: u8, index: u8 },

    // Instance field operations (all format 22c)
    Iget { dest: u4, obj: u4, field: FieldIdx },
    IgetWide { dest: u4, obj: u4, field: FieldIdx },
    IgetObject { dest: u4, obj: u4, field: FieldIdx },
    IgetBoolean { dest: u4, obj: u4, field: FieldIdx },
    IgetByte { dest: u4, obj: u4, field: FieldIdx },
    IgetChar { dest: u4, obj: u4, field: FieldIdx },
    IgetShort { dest: u4, obj: u4, field: FieldIdx },
    Iput { src: u4, obj: u4, field: FieldIdx },
    IputWide { src: u4, obj: u4, field: FieldIdx },
    IputObject { src: u4, obj: u4, field: FieldIdx },
    IputBoolean { src: u4, obj: u4, field: FieldIdx },
    IputByte { src: u4, obj: u4, field: FieldIdx },
    IputChar { src: u4, obj: u4, field: FieldIdx },
    IputShort { src: u4, obj: u4, field: FieldIdx },

    // Static field operations (all format 21c)
    Sget { dest: u8, field: FieldIdx },
    SgetWide { dest: u8, field: FieldIdx },
    SgetObject { dest: u8, field: FieldIdx },
    SgetBoolean { dest: u8, field: FieldIdx },
    SgetByte { dest: u8, field: FieldIdx },
    SgetChar { dest: u8, field: FieldIdx },
    SgetShort { dest: u8, field: FieldIdx },
    Sput { src: u8, field: FieldIdx },
    SputWide { src: u8, field: FieldIdx },
    SputObject { src: u8, field: FieldIdx },
    SputBoolean { src: u8, field: FieldIdx },
    SputByte { src: u8, field: FieldIdx },
    SputChar { src: u8, field: FieldIdx },
    SputShort { src: u8, field: FieldIdx },

    // Invoke (format 35c: up to 5 args, or 3rc: range)
    InvokeVirtual { method: MethodIdx, args: SmallVec<[u4; 5]> },
    InvokeSuper { method: MethodIdx, args: SmallVec<[u4; 5]> },
    InvokeDirect { method: MethodIdx, args: SmallVec<[u4; 5]> },
    InvokeStatic { method: MethodIdx, args: SmallVec<[u4; 5]> },
    InvokeInterface { method: MethodIdx, args: SmallVec<[u4; 5]> },
    InvokeVirtualRange { method: MethodIdx, first_reg: u16, count: u8 },
    InvokeSuperRange { method: MethodIdx, first_reg: u16, count: u8 },
    InvokeDirectRange { method: MethodIdx, first_reg: u16, count: u8 },
    InvokeStaticRange { method: MethodIdx, first_reg: u16, count: u8 },
    InvokeInterfaceRange { method: MethodIdx, first_reg: u16, count: u8 },

    // DEX 038+: invoke-polymorphic, invoke-custom
    InvokePolymorphic { method: MethodIdx, proto: ProtoIdx, args: SmallVec<[u4; 5]> },
    InvokePolymorphicRange { method: MethodIdx, proto: ProtoIdx, first_reg: u16, count: u8 },
    InvokeCustom { call_site: CallSiteIdx, args: SmallVec<[u4; 5]> },
    InvokeCustomRange { call_site: CallSiteIdx, first_reg: u16, count: u8 },

    // Unary operations (format 12x)
    NegInt { dest: u4, src: u4 },
    NotInt { dest: u4, src: u4 },
    NegLong { dest: u4, src: u4 },
    NotLong { dest: u4, src: u4 },
    NegFloat { dest: u4, src: u4 },
    NegDouble { dest: u4, src: u4 },
    IntToLong { dest: u4, src: u4 },
    IntToFloat { dest: u4, src: u4 },
    IntToDouble { dest: u4, src: u4 },
    LongToInt { dest: u4, src: u4 },
    LongToFloat { dest: u4, src: u4 },
    LongToDouble { dest: u4, src: u4 },
    FloatToInt { dest: u4, src: u4 },
    FloatToLong { dest: u4, src: u4 },
    FloatToDouble { dest: u4, src: u4 },
    DoubleToInt { dest: u4, src: u4 },
    DoubleToLong { dest: u4, src: u4 },
    DoubleToFloat { dest: u4, src: u4 },
    IntToByte { dest: u4, src: u4 },
    IntToChar { dest: u4, src: u4 },
    IntToShort { dest: u4, src: u4 },

    // Binary operations (format 23x: dest, a, b)
    AddInt { dest: u8, a: u8, b: u8 },
    SubInt { dest: u8, a: u8, b: u8 },
    MulInt { dest: u8, a: u8, b: u8 },
    DivInt { dest: u8, a: u8, b: u8 },
    RemInt { dest: u8, a: u8, b: u8 },
    AndInt { dest: u8, a: u8, b: u8 },
    OrInt { dest: u8, a: u8, b: u8 },
    XorInt { dest: u8, a: u8, b: u8 },
    ShlInt { dest: u8, a: u8, b: u8 },
    ShrInt { dest: u8, a: u8, b: u8 },
    UshrInt { dest: u8, a: u8, b: u8 },
    AddLong { dest: u8, a: u8, b: u8 },
    SubLong { dest: u8, a: u8, b: u8 },
    MulLong { dest: u8, a: u8, b: u8 },
    DivLong { dest: u8, a: u8, b: u8 },
    RemLong { dest: u8, a: u8, b: u8 },
    AndLong { dest: u8, a: u8, b: u8 },
    OrLong { dest: u8, a: u8, b: u8 },
    XorLong { dest: u8, a: u8, b: u8 },
    ShlLong { dest: u8, a: u8, b: u8 },
    ShrLong { dest: u8, a: u8, b: u8 },
    UshrLong { dest: u8, a: u8, b: u8 },
    AddFloat { dest: u8, a: u8, b: u8 },
    SubFloat { dest: u8, a: u8, b: u8 },
    MulFloat { dest: u8, a: u8, b: u8 },
    DivFloat { dest: u8, a: u8, b: u8 },
    RemFloat { dest: u8, a: u8, b: u8 },
    AddDouble { dest: u8, a: u8, b: u8 },
    SubDouble { dest: u8, a: u8, b: u8 },
    MulDouble { dest: u8, a: u8, b: u8 },
    DivDouble { dest: u8, a: u8, b: u8 },
    RemDouble { dest: u8, a: u8, b: u8 },

    // Binary 2addr (format 12x: dest/a, b -- dest is also first operand)
    AddInt2Addr { dest_a: u4, b: u4 },
    SubInt2Addr { dest_a: u4, b: u4 },
    // ... (all 2addr variants follow same pattern)
    MulInt2Addr { dest_a: u4, b: u4 },
    DivInt2Addr { dest_a: u4, b: u4 },
    RemInt2Addr { dest_a: u4, b: u4 },
    AndInt2Addr { dest_a: u4, b: u4 },
    OrInt2Addr { dest_a: u4, b: u4 },
    XorInt2Addr { dest_a: u4, b: u4 },
    ShlInt2Addr { dest_a: u4, b: u4 },
    ShrInt2Addr { dest_a: u4, b: u4 },
    UshrInt2Addr { dest_a: u4, b: u4 },
    AddLong2Addr { dest_a: u4, b: u4 },
    SubLong2Addr { dest_a: u4, b: u4 },
    MulLong2Addr { dest_a: u4, b: u4 },
    DivLong2Addr { dest_a: u4, b: u4 },
    RemLong2Addr { dest_a: u4, b: u4 },
    AndLong2Addr { dest_a: u4, b: u4 },
    OrLong2Addr { dest_a: u4, b: u4 },
    XorLong2Addr { dest_a: u4, b: u4 },
    ShlLong2Addr { dest_a: u4, b: u4 },
    ShrLong2Addr { dest_a: u4, b: u4 },
    UshrLong2Addr { dest_a: u4, b: u4 },
    AddFloat2Addr { dest_a: u4, b: u4 },
    SubFloat2Addr { dest_a: u4, b: u4 },
    MulFloat2Addr { dest_a: u4, b: u4 },
    DivFloat2Addr { dest_a: u4, b: u4 },
    RemFloat2Addr { dest_a: u4, b: u4 },
    AddDouble2Addr { dest_a: u4, b: u4 },
    SubDouble2Addr { dest_a: u4, b: u4 },
    MulDouble2Addr { dest_a: u4, b: u4 },
    DivDouble2Addr { dest_a: u4, b: u4 },
    RemDouble2Addr { dest_a: u4, b: u4 },

    // Literal operations (format 22s, 22b)
    AddIntLit16 { dest: u4, src: u4, literal: i16 },
    RsubIntLit16 { dest: u4, src: u4, literal: i16 },  // rsub: literal - src
    MulIntLit16 { dest: u4, src: u4, literal: i16 },
    DivIntLit16 { dest: u4, src: u4, literal: i16 },
    RemIntLit16 { dest: u4, src: u4, literal: i16 },
    AndIntLit16 { dest: u4, src: u4, literal: i16 },
    OrIntLit16 { dest: u4, src: u4, literal: i16 },
    XorIntLit16 { dest: u4, src: u4, literal: i16 },
    AddIntLit8 { dest: u8, src: u8, literal: i8 },
    RsubIntLit8 { dest: u8, src: u8, literal: i8 },
    MulIntLit8 { dest: u8, src: u8, literal: i8 },
    DivIntLit8 { dest: u8, src: u8, literal: i8 },
    RemIntLit8 { dest: u8, src: u8, literal: i8 },
    AndIntLit8 { dest: u8, src: u8, literal: i8 },
    OrIntLit8 { dest: u8, src: u8, literal: i8 },
    XorIntLit8 { dest: u8, src: u8, literal: i8 },
    ShlIntLit8 { dest: u8, src: u8, literal: i8 },
    ShrIntLit8 { dest: u8, src: u8, literal: i8 },
    UshrIntLit8 { dest: u8, src: u8, literal: i8 },

    // Payloads (these appear inline in the instruction stream)
    PackedSwitchPayload {
        first_key: i32,
        targets: Vec<i32>,
    },
    SparseSwitchPayload {
        keys: Vec<i32>,
        targets: Vec<i32>,
    },
    FillArrayDataPayload {
        element_width: u16,
        data: Vec<u8>,
    },

    /// Unknown/unrecognized instruction. Preserves raw code units
    /// for forward-compatibility with newer DEX versions.
    RawInstruction {
        code_units: SmallVec<[u16; 5]>,
    },
}

/// 4-bit register number (0-15).
pub type u4 = u8;  // Stored as u8, validated to be < 16

/// 4-bit signed immediate.
pub type i4 = i8;  // Stored as i8, validated to be in [-8, 7]
```

### Branch Offset Representation

Branch offsets in the binary format are relative to the address of the branch instruction itself (measured in 16-bit code units). In the IR, store them as-is. During mutation, when instructions are inserted or removed, ALL branch offsets and payload references in the method MUST be recomputed. This is the single most error-prone part of the writer.

Recommended approach: convert branch offsets to label references internally during mutation, then resolve back to numeric offsets on write.

```rust
/// During mutation, branch targets can be symbolic.
#[derive(Debug, Clone, PartialEq)]
pub enum BranchTarget {
    /// Offset in code units from the branching instruction.
    Offset(i32),
    /// Symbolic label (resolved to offset on write).
    Label(Label),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Label(pub u32);
```

---

## 14. Debug Info

Referenced by `code_item.debug_info_off`. Contains line number and local variable information.

### Header

```
line_start      : ULEB128     // Initial line number
parameters_size : ULEB128     // Number of parameters (not including `this`)
parameter_names[parameters_size] : ULEB128p1  // String index for each param, or NO_INDEX
```

### State Machine Bytecodes

After the header, a sequence of bytecodes drives a state machine:

| Value | Name | Description |
|---|---|---|
| 0x00 | `DBG_END_SEQUENCE` | End of debug info |
| 0x01 | `DBG_ADVANCE_PC` | Advance PC by ULEB128 amount |
| 0x02 | `DBG_ADVANCE_LINE` | Advance line by SLEB128 amount |
| 0x03 | `DBG_START_LOCAL` | Start local: register(ULEB128), name(ULEB128p1), type(ULEB128p1) |
| 0x04 | `DBG_START_LOCAL_EXTENDED` | Same + signature(ULEB128p1) for generics |
| 0x05 | `DBG_END_LOCAL` | End local: register(ULEB128) |
| 0x06 | `DBG_RESTART_LOCAL` | Restart local: register(ULEB128) |
| 0x07 | `DBG_SET_PROLOGUE_END` | Next position entry is after method prologue |
| 0x08 | `DBG_SET_EPILOGUE_BEGIN` | Next position entry is before method epilogue |
| 0x09 | `DBG_SET_FILE` | Set source file: name(ULEB128p1) |
| 0x0A-0xFF | Special opcodes | Encode line + PC advance in one byte |

### Special Opcodes

For values `0x0A` through `0xFF`:
```
adjusted_opcode = opcode - 0x0A
line_advance = (adjusted_opcode % 15) + (-4)   // Range: -4 to 10
pc_advance = adjusted_opcode / 15               // Range: 0 to 16
```

Then: `line += line_advance; pc += pc_advance; emit position entry`.

```rust
#[derive(Debug, Clone)]
pub struct DebugInfo {
    pub line_start: u32,
    pub parameter_names: Vec<Option<StringIdx>>,
    pub bytecodes: Vec<DebugBytecode>,
}

#[derive(Debug, Clone)]
pub enum DebugBytecode {
    EndSequence,
    AdvancePc { advance: u32 },
    AdvanceLine { advance: i32 },
    StartLocal {
        register: u32,
        name: Option<StringIdx>,
        type_: Option<TypeIdx>,
    },
    StartLocalExtended {
        register: u32,
        name: Option<StringIdx>,
        type_: Option<TypeIdx>,
        signature: Option<StringIdx>,
    },
    EndLocal { register: u32 },
    RestartLocal { register: u32 },
    SetPrologueEnd,
    SetEpilogueBegin,
    SetFile { name: Option<StringIdx> },
    /// Special opcode encoding a line + PC advance.
    SpecialAdvance { line_advance: i32, pc_advance: u32 },
}
```

---

## 15. Annotations

### Annotations Directory Item

Referenced by `class_def.annotations_off`. 4-byte aligned.

```
class_annotations_off    : u32     // Offset to annotation_set_item, or 0
fields_size              : u32
annotated_methods_size   : u32
annotated_parameters_size: u32

field_annotations[fields_size]:
    field_idx            : u32
    annotations_off      : u32     // -> annotation_set_item

method_annotations[annotated_methods_size]:
    method_idx           : u32
    annotations_off      : u32     // -> annotation_set_item

parameter_annotations[annotated_parameters_size]:
    method_idx           : u32
    annotations_off      : u32     // -> annotation_set_ref_list
```

### Annotation Set Item

```
size                     : u32
entries[size]            : u32     // Offsets to annotation_item (sorted by type_idx)
```

### Annotation Set Ref List (for parameter annotations)

```
size                     : u32
list[size]               : u32     // Offsets to annotation_set_item (one per parameter)
```

### Annotation Item

```
visibility               : u8      // See Appendix D
annotation               : encoded_annotation  // (see Encoded Values)
```

```rust
#[derive(Debug, Clone)]
pub struct AnnotationsDirectory {
    pub class_annotations: Vec<AnnotationItem>,
    pub field_annotations: Vec<(FieldIdx, Vec<AnnotationItem>)>,
    pub method_annotations: Vec<(MethodIdx, Vec<AnnotationItem>)>,
    pub parameter_annotations: Vec<(MethodIdx, Vec<Vec<AnnotationItem>>)>,
}

#[derive(Debug, Clone)]
pub struct AnnotationItem {
    pub visibility: AnnotationVisibility,
    pub type_: TypeIdx,
    pub elements: Vec<AnnotationElement>,
}

#[derive(Debug, Clone)]
pub struct AnnotationElement {
    pub name: StringIdx,
    pub value: EncodedValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationVisibility {
    Build,   // 0x00 - compile-time only
    Runtime, // 0x01 - visible at runtime
    System,  // 0x02 - visible to system only
}
```

---

## 16. Map List

Located at `header.map_off`. Provides a manifest of all sections in the file.

```
size            : u32
list[size]:
    type        : u16     // Item type code
    unused      : u16     // Always 0
    size        : u32     // Count of items
    offset      : u32     // File offset
```

### Item Type Codes

| Code | Name | Description |
|---|---|---|
| 0x0000 | TYPE_HEADER_ITEM | File header |
| 0x0001 | TYPE_STRING_ID_ITEM | String IDs |
| 0x0002 | TYPE_TYPE_ID_ITEM | Type IDs |
| 0x0003 | TYPE_PROTO_ID_ITEM | Prototype IDs |
| 0x0004 | TYPE_FIELD_ID_ITEM | Field IDs |
| 0x0005 | TYPE_METHOD_ID_ITEM | Method IDs |
| 0x0006 | TYPE_CLASS_DEF_ITEM | Class definitions |
| 0x0007 | TYPE_CALL_SITE_ID_ITEM | Call site IDs (038+) |
| 0x0008 | TYPE_METHOD_HANDLE_ITEM | Method handles (038+) |
| 0x1000 | TYPE_MAP_LIST | The map list itself |
| 0x1001 | TYPE_TYPE_LIST | Type lists |
| 0x1002 | TYPE_ANNOTATION_SET_REF_LIST | Annotation set ref lists |
| 0x1003 | TYPE_ANNOTATION_SET_ITEM | Annotation sets |
| 0x2000 | TYPE_CLASS_DATA_ITEM | Class data |
| 0x2001 | TYPE_CODE_ITEM | Code items |
| 0x2002 | TYPE_STRING_DATA_ITEM | String data |
| 0x2003 | TYPE_DEBUG_INFO_ITEM | Debug info |
| 0x2004 | TYPE_ANNOTATION_ITEM | Annotation items |
| 0x2005 | TYPE_ENCODED_ARRAY_ITEM | Encoded arrays |
| 0x2006 | TYPE_ANNOTATIONS_DIRECTORY_ITEM | Annotations directories |
| 0xF000 | TYPE_HIDDENAPI_CLASS_DATA_ITEM | Hidden API data (039+) |

The writer MUST emit a valid map list that accounts for all sections written. Entries must be sorted by offset.

---

## 17. Encoded Values

Used in annotations, static field initializers, and encoded arrays.

### Format

First byte: `(value_type << 5) | value_arg`

Where `value_arg` indicates size minus one (number of following bytes minus one), except for some types.

| value_type | Name | value_arg meaning | Data |
|---|---|---|---|
| 0x00 | VALUE_BYTE | 0 (must be 0) | 1 byte signed |
| 0x02 | VALUE_SHORT | size-1 (0..1) | 1-2 bytes signed |
| 0x03 | VALUE_CHAR | size-1 (0..1) | 1-2 bytes unsigned |
| 0x04 | VALUE_INT | size-1 (0..3) | 1-4 bytes signed |
| 0x06 | VALUE_LONG | size-1 (0..7) | 1-8 bytes signed |
| 0x10 | VALUE_FLOAT | size-1 (0..3) | 1-4 bytes, right-zero-extended to 4 |
| 0x11 | VALUE_DOUBLE | size-1 (0..7) | 1-8 bytes, right-zero-extended to 8 |
| 0x15 | VALUE_METHOD_TYPE | size-1 (0..3) | ProtoIdx |
| 0x16 | VALUE_METHOD_HANDLE | size-1 (0..3) | MethodHandleIdx |
| 0x17 | VALUE_STRING | size-1 (0..3) | StringIdx |
| 0x18 | VALUE_TYPE | size-1 (0..3) | TypeIdx |
| 0x19 | VALUE_FIELD | size-1 (0..3) | FieldIdx |
| 0x1a | VALUE_METHOD | size-1 (0..3) | MethodIdx |
| 0x1b | VALUE_ENUM | size-1 (0..3) | FieldIdx (enum constant) |
| 0x1c | VALUE_ARRAY | 0 | encoded_array follows |
| 0x1d | VALUE_ANNOTATION | 0 | encoded_annotation follows |
| 0x1e | VALUE_NULL | 0 | (no data) |
| 0x1f | VALUE_BOOLEAN | 0 or 1 | value_arg IS the boolean value |

**Integer encoding**: Values are stored in the minimum number of bytes needed, sign-extended on read. For example, the integer `255` is stored as 2 bytes `[0xFF, 0x00]` because sign extension of a single `0xFF` byte would produce `-1`.

**Float/Double encoding**: Stored in minimum bytes with zeros stripped from the RIGHT (high-order bytes in memory, since little-endian). On read, right-zero-extend to the full width.

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum EncodedValue {
    Byte(i8),
    Short(i16),
    Char(u16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    MethodType(ProtoIdx),
    MethodHandle(MethodHandleIdx),
    String(StringIdx),
    Type(TypeIdx),
    Field(FieldIdx),
    Method(MethodIdx),
    Enum(FieldIdx),
    Array(Vec<EncodedValue>),
    Annotation(EncodedAnnotation),
    Null,
    Boolean(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub struct EncodedAnnotation {
    pub type_: TypeIdx,
    pub elements: Vec<AnnotationElement>,
}
```

### Encoded Array

```
size            : ULEB128
values[size]    : encoded_value
```

---

## 18. Call Site and Method Handle Sections (DEX 038+)

### Call Site IDs

Array of 4-byte offsets to `encoded_array_item` entries, where each array encodes:

1. `METHOD_HANDLE` - the bootstrap linker method
2. `STRING` - the method name to resolve
3. `METHOD_TYPE` - the method type to resolve
4. Additional arguments (0 or more `encoded_value` items)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CallSiteIdx(pub u32);

#[derive(Debug, Clone)]
pub struct CallSiteItem {
    pub bootstrap_method: MethodHandleIdx,
    pub method_name: StringIdx,
    pub method_type: ProtoIdx,
    pub extra_arguments: Vec<EncodedValue>,
}
```

### Method Handles

Each entry is 8 bytes:

| Offset | Size | Field | Description |
|---|---|---|---|
| 0x00 | 2 | `method_handle_type` | Handle type (see below) |
| 0x02 | 2 | `unused` | 0 |
| 0x04 | 2 | `field_or_method_id` | Index into field_ids or method_ids |
| 0x06 | 2 | `unused` | 0 |

### Handle Types

| Value | Name | field_or_method_id points to |
|---|---|---|
| 0x00 | METHOD_HANDLE_TYPE_STATIC_PUT | field_ids |
| 0x01 | METHOD_HANDLE_TYPE_STATIC_GET | field_ids |
| 0x02 | METHOD_HANDLE_TYPE_INSTANCE_PUT | field_ids |
| 0x03 | METHOD_HANDLE_TYPE_INSTANCE_GET | field_ids |
| 0x04 | METHOD_HANDLE_TYPE_INVOKE_STATIC | method_ids |
| 0x05 | METHOD_HANDLE_TYPE_INVOKE_INSTANCE | method_ids |
| 0x06 | METHOD_HANDLE_TYPE_INVOKE_CONSTRUCTOR | method_ids |
| 0x07 | METHOD_HANDLE_TYPE_INVOKE_DIRECT | method_ids |
| 0x08 | METHOD_HANDLE_TYPE_INVOKE_INTERFACE | method_ids |

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MethodHandleIdx(pub u32);

#[derive(Debug, Clone)]
pub struct MethodHandle {
    pub handle_type: MethodHandleType,
    pub member: MethodHandleMember,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodHandleType {
    StaticPut,
    StaticGet,
    InstancePut,
    InstanceGet,
    InvokeStatic,
    InvokeInstance,
    InvokeConstructor,
    InvokeDirect,
    InvokeInterface,
}

#[derive(Debug, Clone)]
pub enum MethodHandleMember {
    Field(FieldIdx),
    Method(MethodIdx),
}
```

---

## 19. HiddenAPI Data (DEX 039+)

Referenced by map list entry `TYPE_HIDDENAPI_CLASS_DATA_ITEM` (0xF000).

### Format

```
offsets[class_defs_size] : u32    // One per class_def. Offset into data[], or 0 if no restrictions.

data[]:                           // Concatenated per-class flag sequences
    // For each class with offset != 0:
    //   For each field and method in the class_data_item (in order:
    //     static_fields, instance_fields, direct_methods, virtual_methods):
    //     flag : ULEB128          // Hidden API restriction flag
```

### Flag Values

| Value | Meaning |
|---|---|
| 0 | No restriction (SDK API) |
| 1 | Greylist |
| 2 | Blacklist |
| 3 | Greylist-max-o (removed in Q) |
| 4 | Greylist-max-p (removed in Q) |
| 5 | Greylist-max-q |
| 6 | Greylist-max-r |

```rust
#[derive(Debug, Clone)]
pub struct HiddenApiData {
    /// One entry per class_def. None means no restrictions for that class.
    pub class_flags: Vec<Option<ClassHiddenApiFlags>>,
}

#[derive(Debug, Clone)]
pub struct ClassHiddenApiFlags {
    pub static_field_flags: Vec<HiddenApiFlag>,
    pub instance_field_flags: Vec<HiddenApiFlag>,
    pub direct_method_flags: Vec<HiddenApiFlag>,
    pub virtual_method_flags: Vec<HiddenApiFlag>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum HiddenApiFlag {
    Sdk = 0,
    Greylist = 1,
    Blacklist = 2,
    GreylistMaxO = 3,
    GreylistMaxP = 4,
    GreylistMaxQ = 5,
    GreylistMaxR = 6,
}
```

---

## 20. In-Memory IR Design

### Design Principles

1. **Index tables own the data.** Strings, types, prototypes, fields, and methods live in index-addressable tables. Everything else references them by index.
2. **Lazy resolution.** Parse the fixed-size ID tables eagerly (fast sequential reads). Parse variable-size data (class data, code items, debug info, annotations) lazily on first access.
3. **Copy-on-write semantics.** Unmodified data references the original buffer. Modifications allocate new owned data.
4. **Interning.** When new strings/types/fields/methods are added, they're interned into the tables automatically. Duplicate detection uses hash maps.

### Top-Level Structure

```rust
/// The main DEX file representation.
pub struct DexFile {
    // Parsed header
    header: DexHeader,

    // Index tables (eagerly parsed)
    strings: IndexMap<StringIdx, DexString>,
    types: IndexMap<TypeIdx, StringIdx>,         // type_idx -> descriptor string_idx
    prototypes: IndexMap<ProtoIdx, Prototype>,
    fields: IndexMap<FieldIdx, FieldId>,
    methods: IndexMap<MethodIdx, MethodId>,

    // Class definitions (eagerly parsed headers, lazily parsed bodies)
    classes: IndexMap<TypeIdx, ClassDef>,

    // DEX 038+ (optional)
    call_sites: Vec<CallSiteItem>,
    method_handles: Vec<MethodHandle>,

    // DEX 039+ (optional)
    hidden_api: Option<HiddenApiData>,

    // Reverse lookup maps (built on demand)
    string_lookup: HashMap<String, StringIdx>,
    type_lookup: HashMap<StringIdx, TypeIdx>,

    // Original raw buffer (for zero-copy and round-trip)
    raw: Option<Arc<[u8]>>,
}
```

### `IndexMap` Semantics

`IndexMap` here refers to an ordered map that preserves insertion order and supports O(1) access by index. Use `indexmap::IndexMap` or a custom `Vec<T>` + `HashMap<K, usize>` combo. The ordering matters because DEX tables have specific sort requirements.

### Lazy Parsing Strategy

```rust
enum LazyData<T> {
    /// Not yet parsed. Contains the offset into the raw buffer.
    Unparsed(u32),
    /// Parsed data.
    Parsed(T),
    /// Data has been modified.
    Modified(T),
}
```

Class data and code items use `LazyData`:

```rust
pub struct ClassDef {
    pub class_type: TypeIdx,
    pub access_flags: AccessFlags,
    pub superclass: Option<TypeIdx>,
    pub interfaces: Vec<TypeIdx>,
    pub source_file: Option<StringIdx>,
    pub(crate) annotations: LazyData<AnnotationsDirectory>,
    pub(crate) class_data: LazyData<ClassData>,
    pub static_values: Vec<EncodedValue>,
}
```

When a patch accesses `class_def.class_data()`, if it's `Unparsed`, parse from the raw buffer, transition to `Parsed`, and return a reference. When mutated, transition to `Modified`.

---

## 21. Writer / Serializer Design

### Write Order

The writer must produce sections in the canonical order. The reference order (matching `dx` and `d8` output):

1. Header (placeholder, filled last)
2. String IDs (placeholder, filled after string data is written)
3. Type IDs
4. Prototype IDs
5. Field IDs
6. Method IDs
7. Class Definitions (placeholder, filled after class data is written)
8. Call Site IDs (if DEX 038+)
9. Method Handles (if DEX 038+)
10. Data section (in this order):
    a. Type lists (4-byte aligned)
    b. Annotation set ref lists (4-byte aligned)
    c. Annotation sets (4-byte aligned)
    d. Class data items (no alignment requirement)
    e. Code items (4-byte aligned)
    f. String data items (no alignment requirement)
    g. Debug info items (no alignment requirement)
    h. Annotation items (no alignment requirement)
    i. Encoded arrays (no alignment requirement)
    j. Annotations directories (4-byte aligned)
    k. Hidden API data (if DEX 039+)
11. Map list (4-byte aligned)
12. Link data (if any)

### Offset Resolution Strategy

Use a two-pass approach:

**Pass 1: Collect and sort.** Gather all items that need to be written. Assign them to their section. Sort each section as required (strings by UTF-16 order, types by descriptor index, etc.). Deduplicate shared data (type lists, annotation sets).

**Pass 2: Write.** Write sections sequentially, recording the offset of each item as it's written. After all data sections are written, go back and fill in:
- String ID offsets (now that string data offsets are known)
- Class def offsets (class_data_off, annotations_off, static_values_off)
- Code item offsets (debug_info_off)
- Prototype parameter list offsets
- Header section sizes and offsets
- Checksum and signature

### Buffer Strategy

```rust
pub struct DexWriter {
    buf: Vec<u8>,
    /// Track where each item was written for back-patching.
    string_data_offsets: Vec<u32>,
    type_list_offsets: HashMap<Vec<TypeIdx>, u32>,  // Deduplicated
    code_item_offsets: Vec<u32>,
    // ... etc
}

impl DexWriter {
    pub fn write(dex: &DexFile) -> Result<Vec<u8>> {
        let mut w = Self::new();
        w.write_header_placeholder()?;
        w.write_string_ids_placeholder(dex)?;
        w.write_type_ids(dex)?;
        w.write_proto_ids(dex)?;
        w.write_field_ids(dex)?;
        w.write_method_ids(dex)?;
        w.write_class_defs_placeholder(dex)?;
        w.write_call_site_ids(dex)?;
        w.write_method_handles(dex)?;
        // Data section
        w.write_type_lists(dex)?;
        w.write_annotation_sets(dex)?;
        w.write_class_data_items(dex)?;
        w.write_code_items(dex)?;
        w.write_string_data(dex)?;
        w.write_debug_info(dex)?;
        w.write_annotations(dex)?;
        w.write_encoded_arrays(dex)?;
        w.write_annotations_directories(dex)?;
        w.write_hidden_api(dex)?;
        w.write_map_list(dex)?;
        // Back-patch
        w.patch_string_ids()?;
        w.patch_class_defs()?;
        w.patch_header()?;
        w.compute_signature()?;
        w.compute_checksum()?;
        Ok(w.buf)
    }
}
```

### Alignment Helper

```rust
impl DexWriter {
    fn align(&mut self, alignment: usize) {
        let padding = (alignment - (self.buf.len() % alignment)) % alignment;
        self.buf.extend(std::iter::repeat(0u8).take(padding));
    }

    fn write_u8(&mut self, v: u8) { self.buf.push(v); }
    fn write_u16(&mut self, v: u16) { self.buf.extend_from_slice(&v.to_le_bytes()); }
    fn write_u32(&mut self, v: u32) { self.buf.extend_from_slice(&v.to_le_bytes()); }
    fn write_i32(&mut self, v: i32) { self.buf.extend_from_slice(&v.to_le_bytes()); }
    fn pos(&self) -> u32 { self.buf.len() as u32 }
}
```

### Deduplication

Several data types can be shared across classes and must be deduplicated:

- **Type lists**: Same parameter list or interface list should be written once.
- **Annotation sets**: Identical annotation sets should be written once.
- **String data**: Guaranteed unique by the string table design (no duplication possible).
- **Debug info**: Typically NOT deduplicated (each method has unique line numbers).

Use `HashMap` keyed by content to track already-written items and reuse their offsets.

---

## 22. Round-Trip Fidelity Requirements

### Strict Round-Trip (no mutations)

`write(parse(bytes)) == bytes` must hold when:

1. No mutations have been applied.
2. The original file was produced by `dx`, `d8`, or `r8` (standard toolchains).
3. `ParseOptions` does not have lenient mode enabled.

This requires:
- Preserving exact LEB128 encoding lengths (even if over-long).
- Preserving exact string data byte sequences (even if MUTF-8 is non-canonical).
- Preserving section ordering and padding.
- Preserving debug info bytecodes exactly.

**Implementation**: When `raw` is `Some` and no data in a section has been modified, copy raw bytes directly from the original buffer rather than re-serializing.

### Semantic Round-Trip (after mutations)

After mutations, byte-identical output is NOT required. But the output must:

1. Be a valid DEX file accepted by all Android runtime versions.
2. Contain all the same classes, methods, fields, and instructions as the input, except where explicitly modified.
3. Pass `dexdump` verification.
4. Have correct checksums and signatures.
5. Have correctly sorted tables and valid cross-references.

---

## 23. Mutation API

### Adding/Removing Strings

```rust
impl DexFile {
    /// Intern a string. If it already exists, return existing index.
    /// If new, it will be assigned an index on next write.
    pub fn intern_string(&mut self, s: &str) -> StringIdx;

    /// Intern a type descriptor. Interns the descriptor string too.
    pub fn intern_type(&mut self, descriptor: &str) -> TypeIdx;

    /// Intern a complete method reference.
    pub fn intern_method(
        &mut self,
        class: &str,        // e.g., "Lcom/example/Foo;"
        name: &str,         // e.g., "bar"
        proto: &str,        // e.g., "(II)V"
    ) -> MethodIdx;

    /// Intern a field reference.
    pub fn intern_field(
        &mut self,
        class: &str,
        name: &str,
        type_: &str,
    ) -> FieldIdx;
}
```

### Modifying Instructions

```rust
impl CodeItem {
    /// Get the instruction at the given index.
    pub fn instruction(&self, index: usize) -> &Instruction;

    /// Replace the instruction at the given index.
    pub fn replace_instruction(&mut self, index: usize, insn: Instruction);

    /// Insert an instruction before the given index.
    /// Updates all branch offsets and exception handler addresses.
    pub fn insert_instruction(&mut self, index: usize, insn: Instruction);

    /// Insert multiple instructions before the given index.
    pub fn insert_instructions(&mut self, index: usize, insns: &[Instruction]);

    /// Remove the instruction at the given index.
    /// Updates all branch offsets and exception handler addresses.
    pub fn remove_instruction(&mut self, index: usize);

    /// Replace the entire instruction list.
    pub fn set_instructions(&mut self, insns: Vec<Instruction>);

    /// Make this method return void immediately.
    pub fn return_early(&mut self) {
        self.set_instructions(vec![Instruction::ReturnVoid]);
    }

    /// Make this method return a constant integer value.
    pub fn return_early_int(&mut self, value: i32) {
        self.set_instructions(vec![
            Instruction::Const { dest: 0, value },
            Instruction::Return { src: 0 },
        ]);
    }
}
```

### Adding Classes

```rust
impl DexFile {
    /// Add a new class definition.
    pub fn add_class(&mut self, class: ClassDef);

    /// Remove a class by type index.
    pub fn remove_class(&mut self, type_: TypeIdx) -> Option<ClassDef>;

    /// Find a class by descriptor string.
    pub fn find_class(&self, descriptor: &str) -> Option<&ClassDef>;

    /// Find a class mutably.
    pub fn find_class_mut(&mut self, descriptor: &str) -> Option<&mut ClassDef>;
}
```

### Fingerprint-Style Searching

```rust
impl DexFile {
    /// Find a method matching a predicate.
    pub fn find_method<F>(&self, predicate: F) -> Option<MethodRef>
    where
        F: Fn(&MethodId, &ClassDef) -> bool;

    /// Find all methods matching a predicate.
    pub fn find_methods<F>(&self, predicate: F) -> Vec<MethodRef>
    where
        F: Fn(&MethodId, &ClassDef) -> bool;

    /// Search for methods containing a specific instruction pattern.
    pub fn find_methods_with_pattern(&self, pattern: &[InstructionPattern]) -> Vec<MethodRef>;
}

/// A reference to a method within a specific class.
pub struct MethodRef<'a> {
    pub class: &'a mut ClassDef,
    pub method: &'a mut EncodedMethod,
}
```

---

## 24. Error Handling Strategy

### Error Type

```rust
#[derive(Debug, thiserror::Error)]
pub enum DexError {
    #[error("Invalid magic bytes: expected dex\\n0NN\\0, got {found:?}")]
    InvalidMagic { found: [u8; 8] },

    #[error("Unsupported DEX version: {version}")]
    UnsupportedVersion { version: String },

    #[error("Checksum mismatch: expected {expected:#010x}, computed {computed:#010x}")]
    ChecksumMismatch { expected: u32, computed: u32 },

    #[error("Signature mismatch at offset {offset}")]
    SignatureMismatch { offset: usize },

    #[error("File truncated: expected {expected} bytes, got {actual}")]
    FileTruncated { expected: usize, actual: usize },

    #[error("Invalid offset {offset:#010x} for {section} (file size: {file_size:#010x})")]
    InvalidOffset { offset: u32, section: &'static str, file_size: u32 },

    #[error("Invalid LEB128 encoding at offset {offset:#010x}: exceeded 5 bytes")]
    InvalidLeb128 { offset: usize },

    #[error("Invalid MUTF-8 encoding at offset {offset:#010x}: {detail}")]
    InvalidMutf8 { offset: usize, detail: String },

    #[error("Invalid opcode {opcode:#04x} at code offset {offset}")]
    InvalidOpcode { opcode: u8, offset: u32 },

    #[error("String table not sorted at index {index} ('{prev}' >= '{current}')")]
    StringTableUnsorted { index: u32, prev: String, current: String },

    #[error("Duplicate class definition for {descriptor}")]
    DuplicateClass { descriptor: String },

    #[error("Missing class data for non-abstract, non-interface class {descriptor}")]
    MissingClassData { descriptor: String },

    #[error("Index out of bounds: {index_type} index {index} >= table size {table_size}")]
    IndexOutOfBounds { index_type: &'static str, index: u32, table_size: u32 },

    #[error("Alignment violation: {section} at offset {offset:#010x} (required: {required}-byte)")]
    AlignmentViolation { section: &'static str, offset: u32, required: u32 },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, DexError>;
```

### Error Philosophy

- Parse errors should be **precise**: include the exact byte offset, the section being parsed, and what was expected vs found.
- Use `Result` everywhere, never panic on malformed input.
- Provide a `DexError::context()` method or use `anyhow`-style context chaining for nested errors.
- In lenient mode, collect warnings in a `Vec<DexWarning>` alongside the parsed result.

---

## 25. Performance Requirements

### Benchmarks (YouTube-scale: ~15MB main DEX, 130MB APK with 5-8 DEX files)

| Operation | Target | Notes |
|---|---|---|
| Parse header + ID tables | < 5ms | Sequential read, trivially fast |
| Parse all class data + code items | < 200ms | Bulk of the work |
| Full parse (all sections) | < 300ms | Including debug info and annotations |
| Write (no mutations) | < 200ms | Raw byte copy for unmodified sections |
| Write (with mutations) | < 400ms | Re-serialize modified sections only |
| Fingerprint search (scan all methods) | < 50ms | Pattern matching over instruction IR |
| Single method mutation | < 1ms | Instruction insert/replace/remove |

### Memory Budget

| State | Target |
|---|---|
| Parsed DEX (all sections, 15MB input) | < 80MB RSS |
| Parsed DEX (lazy, only headers + IDs) | < 30MB RSS |
| Original buffer retained (for round-trip) | +15MB (mmap'd) |

### Optimization Techniques

1. **Memory-mapped I/O**: Use `memmap2` for the input buffer. Avoid copying the entire file into a `Vec<u8>`.
2. **Arena allocation**: Use `bumpalo` or typed-arena for annotations and debug info (many small allocations).
3. **Parallel class parsing**: Use `rayon` to parse class data items in parallel (they're independent).
4. **SmallVec for instructions**: Invoke argument lists are at most 5 elements. Use `SmallVec<[u4; 5]>` to avoid heap allocation.
5. **String interning**: Use a single `HashMap<&str, StringIdx>` for fast duplicate detection during write.

---

## 26. Testing Strategy

### Level 1: Unit Tests

- LEB128 encode/decode round-trip for edge values: 0, 1, 127, 128, 16383, 16384, max u32, negative values for SLEB128.
- MUTF-8 encode/decode for: ASCII, BMP characters, supplementary characters (surrogate pairs), null character, empty string.
- Instruction encode/decode for every opcode.
- Encoded value encode/decode for every value type.

### Level 2: Integration Tests (Round-Trip)

For each test APK:
1. Extract DEX files.
2. Parse each DEX.
3. Write back without mutations.
4. Assert byte-identical output.

**Test corpus** (minimum):
- Empty DEX (one empty class).
- `dx`-compiled simple Java program.
- `d8`-compiled Kotlin program.
- YouTube APK's `classes.dex` through `classes8.dex`.
- ProGuard/R8-obfuscated APK.
- APK with annotations (Dagger, Room, etc.).
- APK using invoke-custom/invoke-polymorphic (Java 8+ lambdas compiled with D8).
- APK with hidden API metadata (framework DEX from AOSP).

### Level 3: Mutation Tests

1. Parse DEX, add a new class with a method, write, verify with `dexdump`.
2. Parse DEX, find a method, replace an instruction, write, verify.
3. Parse DEX, insert instructions that change branch offsets, write, verify all branches still land correctly.
4. Parse DEX, add new strings/types/fields/methods, write, verify table sorting.
5. Parse DEX, remove a class, write, verify no dangling references.

### Level 4: Fuzz Testing

Use `cargo-fuzz` with `libfuzzer`:
- Fuzz the parser with arbitrary byte sequences. Must not panic.
- Fuzz the parser with mutated valid DEX files (bit flips, truncation, section swaps).
- Fuzz the LEB128 decoder with arbitrary byte sequences.
- Property-based testing: for any valid `DexFile` IR, `parse(write(ir))` produces an equivalent IR.

### Level 5: Compatibility Testing

- Take the output DEX files and load them with `dexlib2` (via a Java test harness). Verify dexlib2 can parse them and produces equivalent Smali output.
- Run output DEX files through `d8 --no-desugaring` to verify they pass D8's verification.
- Run output APKs on Android emulators (API 21, 26, 30, 34) to verify runtime loading.

---

## 27. Crate Structure and Public API Surface

### Crate Layout

```
dex-rs/
├── Cargo.toml
├── src/
│   ├── lib.rs                  // Re-exports public API
│   ├── error.rs                // DexError, Result
│   ├── encoding/
│   │   ├── mod.rs
│   │   ├── leb128.rs           // LEB128 encode/decode
│   │   ├── mutf8.rs            // MUTF-8 encode/decode
│   │   └── encoded_value.rs    // EncodedValue encode/decode
│   ├── model/
│   │   ├── mod.rs
│   │   ├── dex_file.rs         // DexFile top-level struct
│   │   ├── header.rs           // DexHeader, DexVersion
│   │   ├── string.rs           // DexString, StringIdx
│   │   ├── types.rs            // TypeIdx, TypeDescriptor
│   │   ├── proto.rs            // ProtoIdx, Prototype
│   │   ├── field.rs            // FieldIdx, FieldId
│   │   ├── method.rs           // MethodIdx, MethodId
│   │   ├── class.rs            // ClassDef, ClassData, EncodedField, EncodedMethod
│   │   ├── code.rs             // CodeItem, TryItem, CatchHandler
│   │   ├── instruction.rs      // Instruction enum, BranchTarget
│   │   ├── debug.rs            // DebugInfo, DebugBytecode
│   │   ├── annotation.rs       // AnnotationsDirectory, AnnotationItem
│   │   ├── call_site.rs        // CallSiteIdx, CallSiteItem
│   │   ├── method_handle.rs    // MethodHandleIdx, MethodHandle
│   │   ├── hidden_api.rs       // HiddenApiData, HiddenApiFlag
│   │   └── access_flags.rs     // AccessFlags bitflags
│   ├── reader/
│   │   ├── mod.rs
│   │   ├── parse.rs            // Top-level parse() function
│   │   ├── header_reader.rs
│   │   ├── id_reader.rs        // String/Type/Proto/Field/Method ID readers
│   │   ├── class_reader.rs     // ClassDef + ClassData reader
│   │   ├── code_reader.rs      // CodeItem + instruction decoder
│   │   ├── debug_reader.rs
│   │   ├── annotation_reader.rs
│   │   └── map_reader.rs
│   ├── writer/
│   │   ├── mod.rs
│   │   ├── write.rs            // Top-level write() function
│   │   ├── header_writer.rs
│   │   ├── id_writer.rs
│   │   ├── class_writer.rs
│   │   ├── code_writer.rs      // Instruction encoder + branch fixup
│   │   ├── debug_writer.rs
│   │   ├── annotation_writer.rs
│   │   ├── map_writer.rs
│   │   └── checksum.rs         // Adler-32 + SHA-1
│   └── util/
│       ├── mod.rs
│       ├── sort.rs             // DEX table sorting utilities
│       ├── dedup.rs            // Type list / annotation set deduplication
│       └── descriptor.rs       // Type descriptor parsing/validation
├── tests/
│   ├── round_trip.rs
│   ├── mutation.rs
│   ├── edge_cases.rs
│   └── fixtures/               // Test DEX files
│       ├── minimal.dex
│       ├── hello_world.dex
│       ├── annotations.dex
│       ├── invoke_custom.dex
│       └── hidden_api.dex
├── benches/
│   ├── parse.rs
│   └── write.rs
└── fuzz/
    ├── Cargo.toml
    └── fuzz_targets/
        ├── parse.rs
        └── leb128.rs
```

### Public API (lib.rs)

```rust
//! # dex-rs
//!
//! A high-performance DEX file parser, writer, and mutator for Rust.
//!
//! ## Quick Start
//!
//! ```rust
//! use dex_rs::{DexFile, ParseOptions};
//!
//! // Parse
//! let bytes = std::fs::read("classes.dex")?;
//! let dex = DexFile::parse(&bytes, ParseOptions::default())?;
//!
//! // Query
//! for class in dex.classes() {
//!     println!("{}", dex.type_descriptor(class.class_type));
//! }
//!
//! // Mutate
//! let mut dex = dex;
//! let class = dex.find_class_mut("Lcom/example/Target;").unwrap();
//! let method = class.find_method_mut("doSomething").unwrap();
//! method.code_mut().unwrap().return_early();
//!
//! // Write
//! let output = dex.write()?;
//! std::fs::write("patched.dex", output)?;
//! ```

pub mod encoding;
pub mod model;
pub mod reader;
pub mod writer;
pub mod error;

// Re-export commonly used types at crate root
pub use error::{DexError, Result};
pub use model::{
    DexFile, DexHeader, DexVersion, ParseOptions,
    StringIdx, TypeIdx, ProtoIdx, FieldIdx, MethodIdx,
    CallSiteIdx, MethodHandleIdx,
    ClassDef, ClassData, EncodedField, EncodedMethod,
    CodeItem, Instruction, BranchTarget, Label,
    DebugInfo, AnnotationsDirectory, AnnotationItem,
    EncodedValue, AccessFlags,
};
pub use reader::parse;
pub use writer::write;
```

### Feature Flags

```toml
[features]
default = ["mmap", "parallel"]
mmap = ["memmap2"]           # Memory-mapped file I/O
parallel = ["rayon"]         # Parallel class parsing
arena = ["bumpalo"]          # Arena allocation for annotations
serde = ["dep:serde"]        # Serialize/deserialize IR to JSON (for debugging)
```

---

## 28. Known Edge Cases and Pitfalls

### 1. Over-long LEB128

Some obfuscators (and older versions of `dx`) produce LEB128 values with unnecessary trailing bytes. Example: the value `0` encoded as `0x80 0x00` (2 bytes) instead of `0x00` (1 byte). The parser must accept these. The writer should produce canonical encodings.

### 2. Empty class_data

A class can have `class_data_off = 0` even if it's not an interface/abstract class. This means the class has no fields or methods. This is valid for marker interfaces and annotation types.

### 3. Duplicate type lists

Multiple prototypes and classes can reference identical type lists. The writer must deduplicate them (write once, reference multiple times).

### 4. Static values array shorter than static fields

The `static_values` encoded array can have fewer entries than the number of static fields. Missing entries default to zero/null for their type. The writer should omit trailing default values.

### 5. Branch target into middle of instruction

Some obfuscated DEX files contain branch targets that point into the middle of a multi-unit instruction. The ART runtime handles this by treating it as an illegal instruction and throwing a VerifyError at load time. The parser should not crash on this; it should flag it as a warning.

### 6. Annotations referencing deleted elements

After removing a class or method, any annotations that reference the removed element's index become dangling. The writer must either remove such annotations or produce an error.

### 7. Max 65535 type IDs and proto IDs

These are 16-bit indices. If merging DEX files would exceed this limit, the operation must fail with a clear error.

### 8. Endianness of fill-array-data

The fill-array-data payload stores elements in their native byte order (little-endian). For multi-byte elements (short, int, long), the data is packed as little-endian values.

### 9. Debug info references to deleted strings

If a string is removed from the string table, debug info entries referencing it become invalid. The writer should either strip affected debug info entries or remap string indices.

### 10. Compact DEX (CDEX) magic

CDEX files start with `cdex` instead of `dex\n`. The parser should detect this and return a specific error rather than failing with "invalid magic". CDEX support is future work (Section 29).

### 11. `NO_INDEX` as ULEB128p1

In encoded methods, `code_off = 0` means no code (abstract/native). In other contexts, `NO_INDEX` is `0xFFFFFFFF` which encodes as ULEB128p1 value `0` (since 0 - 1 = -1 = 0xFFFFFFFF). Be careful not to confuse "0 offset" with "no index".

### 12. Dalvik vs ART instruction verification

Dalvik (pre-5.0) and ART have slightly different verification rules. For maximum compatibility, the writer should produce DEX files that pass ART's stricter verification.

---

## 29. CDEX / VDEX Compact DEX (Future)

Compact DEX (CDEX) is an on-device format used in VDEX files. It is NOT produced by standard build tools but is found in pre-optimized system images.

### Key Differences from Standard DEX

- Magic: `cdex001\0` (CDEX version 001)
- Header has additional fields after the standard DEX header (feature flags, debug info offsets base, etc.)
- Code items may share debug info via "debug info offsets table" (a separate section that provides base offsets, with per-method deltas)
- Data section may be stored externally (in the VDEX container)
- Some instruction optimization (e.g., method index compression)

### Recommended Approach

Implement as a separate `CdexFile` type that shares the core IR with `DexFile` but has its own parser/writer. The instruction IR and class model should be identical. The differences are purely in the serialization format.

---

## Appendix A: Complete Opcode Table

| Opcode | Mnemonic | Format | Description |
|---|---|---|---|
| 0x00 | nop | 10x | No operation (also used for payload alignment) |
| 0x01 | move | 12x | Move register (non-object, non-wide) |
| 0x02 | move/from16 | 22x | Move from 16-bit register reference |
| 0x03 | move/16 | 32x | Move between 16-bit register refs |
| 0x04 | move-wide | 12x | Move wide (64-bit) |
| 0x05 | move-wide/from16 | 22x | Move wide from 16-bit reg |
| 0x06 | move-wide/16 | 32x | Move wide between 16-bit regs |
| 0x07 | move-object | 12x | Move object reference |
| 0x08 | move-object/from16 | 22x | Move object from 16-bit reg |
| 0x09 | move-object/16 | 32x | Move object between 16-bit regs |
| 0x0a | move-result | 11x | Move result of invoke (non-object) |
| 0x0b | move-result-wide | 11x | Move wide result of invoke |
| 0x0c | move-result-object | 11x | Move object result of invoke |
| 0x0d | move-exception | 11x | Move caught exception to register |
| 0x0e | return-void | 10x | Return from void method |
| 0x0f | return | 11x | Return value |
| 0x10 | return-wide | 11x | Return wide value |
| 0x11 | return-object | 11x | Return object reference |
| 0x12 | const/4 | 11n | 4-bit signed constant |
| 0x13 | const/16 | 21s | 16-bit signed constant |
| 0x14 | const | 31i | 32-bit constant |
| 0x15 | const/high16 | 21h | 16-bit constant << 16 |
| 0x16 | const-wide/16 | 21s | 16-bit signed wide constant |
| 0x17 | const-wide/32 | 31i | 32-bit signed wide constant |
| 0x18 | const-wide | 51l | 64-bit constant |
| 0x19 | const-wide/high16 | 21h | 16-bit constant << 48 |
| 0x1a | const-string | 21c | String constant (16-bit index) |
| 0x1b | const-string/jumbo | 31c | String constant (32-bit index) |
| 0x1c | const-class | 21c | Class constant |
| 0x1d | monitor-enter | 11x | Acquire monitor |
| 0x1e | monitor-exit | 11x | Release monitor |
| 0x1f | check-cast | 21c | Type check with exception |
| 0x20 | instance-of | 22c | Type check to boolean |
| 0x21 | array-length | 12x | Get array length |
| 0x22 | new-instance | 21c | Allocate new object |
| 0x23 | new-array | 22c | Allocate new array |
| 0x24 | filled-new-array | 35c | Allocate + fill array |
| 0x25 | filled-new-array/range | 3rc | Allocate + fill array (range) |
| 0x26 | fill-array-data | 31t | Fill array from payload |
| 0x27 | throw | 11x | Throw exception |
| 0x28 | goto | 10t | Unconditional branch (8-bit offset) |
| 0x29 | goto/16 | 20t | Unconditional branch (16-bit offset) |
| 0x2a | goto/32 | 30t | Unconditional branch (32-bit offset) |
| 0x2b | packed-switch | 31t | Switch (packed keys) |
| 0x2c | sparse-switch | 31t | Switch (sparse keys) |
| 0x2d-0x31 | cmpkind | 23x | Compare float/double/long |
| 0x32-0x37 | if-test | 22t | Two-register conditional |
| 0x38-0x3d | if-testz | 21t | One-register vs zero conditional |
| 0x3e-0x43 | (unused) | 10x | Reserved |
| 0x44-0x51 | arrayop | 23x | Array get/put operations |
| 0x52-0x5f | iinstanceop | 22c | Instance field get/put |
| 0x60-0x6d | sstaticop | 21c | Static field get/put |
| 0x6e-0x72 | invoke-kind | 35c | Method invocation |
| 0x73 | (unused) | 10x | Reserved |
| 0x74-0x78 | invoke-kind/range | 3rc | Method invocation (range) |
| 0x79-0x7a | (unused) | 10x | Reserved |
| 0x7b-0x8f | unop | 12x | Unary operations |
| 0x90-0xaf | binop | 23x | Binary operations |
| 0xb0-0xcf | binop/2addr | 12x | Binary 2-address operations |
| 0xd0-0xd7 | binop/lit16 | 22s | Binary with 16-bit literal |
| 0xd8-0xe2 | binop/lit8 | 22b | Binary with 8-bit literal |
| 0xe3-0xf9 | (unused) | 10x | Reserved |
| 0xfa | invoke-polymorphic | 45cc | Polymorphic invoke (038+) |
| 0xfb | invoke-polymorphic/range | 4rcc | Polymorphic invoke range (038+) |
| 0xfc | invoke-custom | 35c | Custom invoke (038+) |
| 0xfd | invoke-custom/range | 3rc | Custom invoke range (038+) |
| 0xfe | const-method-handle | 21c | Method handle constant (038+) |
| 0xff | const-method-type | 21c | Method type constant (038+) |

---

## Appendix B: Type Descriptor Grammar

```
TypeDescriptor := PrimitiveType | ClassType | ArrayType
PrimitiveType  := 'V' | 'Z' | 'B' | 'S' | 'C' | 'I' | 'J' | 'F' | 'D'
ClassType      := 'L' FullClassName ';'
FullClassName  := SimpleName ('/' SimpleName)*
SimpleName     := [A-Za-z_$] [A-Za-z0-9_$-]*    (in practice, nearly any non-/; char)
ArrayType      := '[' TypeDescriptor

ShortyDescriptor := ShortyReturn ShortyParam*
ShortyReturn     := 'V' | 'Z' | 'B' | 'S' | 'C' | 'I' | 'J' | 'F' | 'D' | 'L'
ShortyParam      := 'Z' | 'B' | 'S' | 'C' | 'I' | 'J' | 'F' | 'D' | 'L'

MethodDescriptor := '(' TypeDescriptor* ')' TypeDescriptor
```

| Descriptor | Type | Size (registers) |
|---|---|---|
| `V` | void | 0 (return only) |
| `Z` | boolean | 1 |
| `B` | byte | 1 |
| `S` | short | 1 |
| `C` | char | 1 |
| `I` | int | 1 |
| `J` | long | 2 (wide) |
| `F` | float | 1 |
| `D` | double | 2 (wide) |
| `L...;` | object ref | 1 |
| `[...` | array ref | 1 |

---

## Appendix C: Access Flag Definitions

```rust
bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct AccessFlags: u32 {
        const PUBLIC       = 0x0001;
        const PRIVATE      = 0x0002;
        const PROTECTED    = 0x0004;
        const STATIC       = 0x0008;
        const FINAL        = 0x0010;
        const SYNCHRONIZED = 0x0020;  // method only
        const VOLATILE     = 0x0040;  // field only
        const BRIDGE       = 0x0040;  // method only (same bit as VOLATILE)
        const TRANSIENT    = 0x0080;  // field only
        const VARARGS      = 0x0080;  // method only (same bit as TRANSIENT)
        const NATIVE       = 0x0100;
        const INTERFACE    = 0x0200;
        const ABSTRACT     = 0x0400;
        const STRICT       = 0x0800;  // strictfp
        const SYNTHETIC    = 0x1000;
        const ANNOTATION   = 0x2000;
        const ENUM         = 0x4000;
        const CONSTRUCTOR  = 0x10000; // method only (DEX-specific)
        const DECLARED_SYNCHRONIZED = 0x20000; // method only (DEX-specific)
    }
}
```

Note: Some flags share the same bit but have different meanings for fields vs methods. The IR should provide convenience methods:

```rust
impl AccessFlags {
    pub fn is_volatile_field(self) -> bool { self.contains(Self::VOLATILE) }
    pub fn is_bridge_method(self) -> bool { self.contains(Self::BRIDGE) }
    pub fn is_transient_field(self) -> bool { self.contains(Self::TRANSIENT) }
    pub fn is_varargs_method(self) -> bool { self.contains(Self::VARARGS) }
}
```

---

## Appendix D: Annotation Visibility Constants

| Value | Name | Meaning |
|---|---|---|
| 0x00 | VISIBILITY_BUILD | Compile-time only (not present at runtime) |
| 0x01 | VISIBILITY_RUNTIME | Available via reflection at runtime |
| 0x02 | VISIBILITY_SYSTEM | Available to the runtime/system, not via user reflection |

---

## Appendix E: Reference Comparison with dexlib2

| Feature | dexlib2 | dex-rs |
|---|---|---|
| Language | Java | Rust |
| Parsing | Eager, full file | Lazy headers, eager IDs |
| String storage | Java String (UTF-16 internally) | Rust String (UTF-8) + MUTF-8 raw for round-trip |
| Instruction representation | Smali text (serialization-heavy) | Typed enum (zero serialization) |
| Memory model | JVM heap, GC'd | Arena/stack, zero-copy where possible |
| Mutation | Modify Smali lists, re-encode | Typed builders, automatic fixup |
| Branch offset fixup | Manual (caller's responsibility) | Automatic on write |
| Thread safety | Largely not thread-safe | `Send + Sync` where appropriate |
| Error handling | Java exceptions (often unchecked) | `Result<T, DexError>` everywhere |
| Round-trip guarantee | No (re-serialized Smali differs) | Yes (raw byte copy for unmodified sections) |
| DEX 038+ support | Full | Full |
| DEX 039+ (hidden API) | Partial | Full |
| CDEX support | No | Future (Section 29) |

---

*End of specification.*
