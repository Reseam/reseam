# reseam-hermes

Borrowed Hermes execution-bytecode parsing and streamed writing. Currently supports
bytecode version 98, from Hermes tag `250829098.0.0-stable`.

`HermesFile::parse` borrows the caller's bytes; keep the input mapping alive and
unchanged. Function bodies and structured sections stay in that mapping. Function
lookup combines names, referenced strings, and JavaScript parameter counts (which
exclude `this`), and reports both missing and ambiguous matches.

`Editor` owns additions over the parsed file. Interning preserves existing IDs and
supports identifier hashes, string kind runs, UTF-16, and overflow table entries.
Writing copies untouched data directly to `Write`, relocates function bodies and
large headers, promotes small headers when offsets exceed their bit fields, moves
the debug section, and recomputes the SHA-1 footer. Writing without edits preserves
the original bytes, including padding and trailing data.

Opcode metadata is generated at build time from the vendored v98
`BytecodeList.def`; instruction decoding uses its operand widths and ID annotations.
Reseam adds annotations for buffer offsets, regexp and shape identities and fills
missing upstream property/class annotations without changing opcode layouts.
The vendored file is licensed under MIT. The crate has one error type, `HermesError`,
for format, lookup, unsupported-version, and IO failures.

`Editor::link` merges a compiled module whose last expression returns its exports
object. Keep declarations inside an IIFE. Linking remaps instructions, switch
tables, exception handlers, string references inside literal buffers, object
shapes, bigint storage and regexp storage. Narrow ID operands grow to their long
variants, with branch targets relocated. Extension debug information is dropped.
A generated bootstrap initializes modules before calling the app's global code,
holding their results in a private lexical environment.

`Editor::wrap` replaces a normal app function with a call to a declared export,
passing a receiver-bound original and every supplied argument. The original body
remains a borrowed source range, including switch payloads, and retains its
captured environment and exceptions. Private export storage is carried only
through the necessary closure ancestry. Environment analysis merges control-flow
paths conservatively and refuses ambiguous depths or spilled scopes it cannot
resolve. Generators, async functions, class constructors, `new.target`, direct
eval and repeated wrapping fail explicitly. Wrapped ordinary functions cannot
subsequently be constructed. Exports must be unconditional literal assignments
of function expressions to the returned exports object.

Linking and wrapping roll back their changes on errors. `into_edits` and `resume`
let a host retain owned edits beside its source mapping without a self-referential
model. Resume requires the original file footer, and the mapping must stay
immutable. Interned additions are indexed without allocating copies of app strings.
