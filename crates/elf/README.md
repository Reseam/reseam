# reseam-elf

Finds functions in little-endian AArch64 and 32-bit ARM ELF shared libraries and edits their
code. Other machine types and big-endian files fail with an error.

- `Editor::new` validates the library and reads its loaded segments and sections, through the
  field layout of its class (`class.rs`). The library stays in its storage, which must not change
  while the editor lives.
- `Editor::find_function` returns the function matching a `Query`: an exported symbol, the
  strings its code references and byte patterns (`Pattern`, hex with `??` wildcards) its code
  holds. It takes the only match, or the one at an index in start-address order. Where functions
  lie is read once, on the first lookup by string or pattern (`code.rs`). The first lookup by
  string builds an index of every data address the code computes, with the function
  computing it (`references.rs`), so later lookups are searches. Registers are followed through
  the instructions that compute an address, and an instruction that overwrites a followed
  register ends its trail.
  - AArch64 (`a64/`): addresses come from `adr`, `adrp`, immediate `add` and scaled loads, and
    the unwind table (`.eh_frame_hdr`) bounds every function.
  - 32-bit ARM (`arm/`): addresses come from a distance loaded as a literal or with `movw` and
    `movt`, plus the program counter. `.ARM.exidx` merges neighbouring functions and has no
    sizes or instruction sets, so the code is walked from the functions the symbols, relocated
    pointers and unwind table record (`discovery.rs`). The walk follows branches and branch
    tables, takes call targets and computed pointers as functions, and decodes A32 (`a32/`) and
    Thumb (`t32/`).
- `Editor::find_branch` returns the one conditional branch whose bytes a pattern's match in a
  function covers. `always_take` replaces it with an unconditional jump to its target and
  `never_take` with a `nop`, each of the branch's size in its instruction set.
- `Editor::always_return` replaces a function with a return of a constant in the first result
  register, in the function's instruction set. AArch64 code keeps a leading `bti` or `paciasp`
  landing pad so indirect calls stay valid, and authenticates the return address a `paciasp` or
  `pacibsp` signed before returning. `replace_bytes` writes raw bytes at an offset inside a
  function. An edit reaching past the function fails.
- `Editor::write` streams the library with every edit overlaid; an unedited library is written
  byte for byte.

Savepoints undo whole patches: `open_savepoint`, `release_savepoint` and `rollback_to` keep and
discard edits made since a point.
