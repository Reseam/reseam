# reseam-hermes

Links JavaScript modules into a React Native app's Hermes bytecode and hooks its functions.
Supports bytecode version 98, from Hermes tag `250829098.0.0-stable`.

- `Editor::new` validates the app's bundle and indexes its functions once, in parallel. The
  bundle stays in its storage, which must not change while the editor lives.
- `Editor::find_function` finds exactly one app function by name, referenced strings and
  parameter count (without `this`). No match or several matches fail with the query.
- `Editor::link` links a compiled module. It evaluates to `{ imports, link }`, keeps its
  declarations in its own scope (an IIFE), and imports app modules by Metro specifier.
  `imports` lists those specifiers. Every reference of the module is remapped and its debug
  information dropped. A bootstrap runs `link` for each module before the app's global code
  and keeps the exports in a private environment.
- `Editor::hook` adds a layer to an app function: code that runs before it, after it, or
  instead of it, and can call linked exports. The original body stays a range of the source.
  Only the closures between the app's global code and the hooked function are rewritten to
  carry the private environment.
- `Editor::write` streams the file. Unedited sections and bodies are copied from the source,
  headers and debug information are relocated, and the SHA-1 footer is recomputed. A file
  without edits is written byte for byte.

Linking and hooking undo themselves when they fail. Savepoints undo whole patches.

Generators, async functions, constructors, functions using `new.target` or direct `eval`, and
environment chains the analysis cannot prove fail with `HermesError::Unsupported`. Hooked
functions cannot be constructed afterwards.

## Modules

| Module | Contents |
|---|---|
| `editor` | `Editor`: the file, its index, analysis caches and edits |
| `parse`, `file`, `layout`, `string`, `function`, `bytes` | the file format: header, sections, strings, function headers |
| `opcode`, `switch`, `literal` | instructions, switch tables and literal buffers |
| `index` | function search by name, strings and parameters |
| `dataflow`, `closure`, `metro`, `interface` | register analyses: closure environments, Metro modules, a module's declared interface |
| `edit` | the edit model: added strings, edited and appended functions, linked modules, undo |
| `link` | linking a module and generating the bootstrap |
| `hook`, `code` | hook code, its checks and code generation |
| `assemble`, `write` | re-encoding edited functions and streaming the output |

Opcode metadata is generated at build time from the vendored v98 `BytecodeList.def`: each
opcode's operands, ID annotations, written registers and wider encodings. The definition does
not record writes, so `build.rs` lists the opcodes whose leading register is only read and those
that write more, from the v98 interpreter. The vendored files are MIT licensed.
