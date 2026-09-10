# Coming from ReVanced

The shape is deliberately close. What differs is that Reseam resolves everything itself: no registers, no DEX file names, no `context`.

| ReVanced | Reseam |
|---|---|
| `bytecodePatch(name, description) { }` | `patch(name) { description(...) }` |
| `rawResourcePatch`, `resourcePatch` | The same `patch { }`; `manifest`, `resources`, `files` are always available. |
| `compatibleWith("pkg"("1.0"))` | Same. |
| `dependsOn(otherPatch)` | Same, and a patch without a name is hidden from users. |
| `val x by stringOption(key, default)` | `val x = stringOption(key, default = ...)` inside the block; read with `options[x]`. |
| `fingerprint { strings(...); returns("Z") }` | `method(label) { strings(...); returns(Type.Boolean) }` |
| `fingerprint { parameters("L...", "I") }` | `params("com.x.Y", Type.Int)` |
| `fingerprint { opcodes(...) }` | `opcode(...)` on the method, or a `point { }` for a position. |
| `fingerprint { custom { method, classDef -> } }` | `rankBy`, `callsMethod { }`, `methodTarget { }` for the rest. |
| `fingerprint.method` | `target.method` |
| `fingerprint.classDef` | `target.method.classDef`, or a `klass` target. |
| `fingerprint.patternMatch.startIndex` | `target.point { }.index` |
| `method.addInstructions(0, "...")` (smali) | `target.before { }` with values, or `target.method.addInstructions(0) { }` with the builder. |
| `method.returnEarly(true)` | `target.alwaysReturn(true)`, or `returnTrueWhen(toggle)` for a setting. |
| `indexOfFirstInstructionOrThrow { }` | `target.point { }`, or `method.indexOfFirstInstruction { }`. |
| `getInstruction<T>(index)` | `method.instructions[index]` with the `Instruction.*` accessors. |
| `extendWith("extensions/x.rve")` | Nothing. Declare an `ExtClass`; the DEX links itself when referenced. |
| Extension `Extension.java` calls | `call(Ext.method, args)` in a code block. |
| `Settings` in the extension | `object AppSettings { val x by toggle(...) }` plus a `settingsHost`; gates with `skipWhen`, `before(toggle)`. |

Two habits to drop:

- Registers. `before { call(Ext.init, thisObject) }` replaces `addInstructions(0, "invoke-static {p0}, ...")`. The engine picks free registers and grows the frame.
- Fingerprints defined inside the patch. Targets are top-level values below the patch, shared between patches by import.

One habit to keep: a fingerprint that fails is a build you fix, not a silent miss. Reseam fails a patch on no match and on more than one match.
