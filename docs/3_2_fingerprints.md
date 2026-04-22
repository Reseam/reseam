# Fingerprints

![Diagram: the same method in two different app releases. Version 19 has it named xyz(); version 20 has it renamed to q(). Both methods still reference the string literals "app_launch_reported" and "session_id" and return type Z. A fingerprint block declaring those strings and that return type matches both versions, so a patch keyed on the fingerprint keeps working across the rename.](fingerprint-match.svg)

Obfuscated method names change between app versions. A fingerprint matches a method by stable properties (strings it references, return type, parameters, opcode shape) so patches survive renames.

```kotlin
import app.reseam.patch.fingerprint

private val shouldReportFingerprint = fingerprint {
    strings("app_launch_reported", "session_id")
    returnType("Z")
}
```

## Builder methods

- `name(string)`: optional label used in error messages.
- `definingClass(descriptor)`: restrict to one class, e.g. `"Lcom/example/Api;"`.
- `accessFlags(flags)`: bitwise combination from `AccessFlags`.
- `returnType(descriptor)`.
- `parameters(vararg)` / `parameterTypes(vararg)`.
- `strings(vararg)`: string literals the method must reference.
- `literal(value)`: numeric literal the method must reference. Call multiple times for multiple literals.
- `opcodes(vararg Int?)`: opcode sequence the method body must contain. `null` matches any opcode at that position.
- `instructions(vararg predicates)`: fine-grained instruction-sequence matcher. Each predicate takes an `Instruction` and returns `Boolean`.
- `custom { ... }`: arbitrary filter with access to the matched method's context (see below).

Example combining several filters:

```kotlin
import app.reseam.patch.AccessFlags
import app.reseam.patch.Opcodes

private val apiCallFingerprint = fingerprint {
    name("ApiCall")
    definingClass("Lcom/example/Api;")
    accessFlags(AccessFlags.PUBLIC or AccessFlags.STATIC)
    returnType("Ljava/lang/String;")
    parameters("Ljava/lang/String;", "I")
    strings("api.example.com", "Bearer")
    literal(200L)
    opcodes(Opcodes.CONST_STRING, Opcodes.INVOKE_STATIC)
}
```

## Custom filters

`custom` blocks run against a `FingerprintMatchContext` exposing `methodName`, `definingClass`, `returnType`, `parameterTypes`, `proto`, `accessFlags`, `registerCount`, `insSize`, `outsSize`, and the resolved `method` / `instructions`:

```kotlin
private val initFingerprint = fingerprint {
    returnType("V")
    custom {
        methodName.startsWith("init") && instructions.size in 10..50
    }
}
```

Multiple `custom` blocks combine with logical AND.

## Instruction-sequence predicates

`instructions(...)` takes predicates over individual instructions. The fingerprint resolves when the predicate sequence matches somewhere inside the method:

```kotlin
import app.reseam.patch.Instruction

private val patchedStringFingerprint = fingerprint {
    returnType("Ljava/lang/String;")
    instructions(
        { (this as? Instruction.RegString)?.value0?.value == "patched" },
    )
}
```

## Resolving

```kotlin
if (!apiCallFingerprint.matched) {
    ctx.log.warn("apiCallFingerprint did not match; skipping")
    return@execute
}

val method = apiCallFingerprint.method
val matchedIndices = apiCallFingerprint.matchedInstructionIndices
```

Always check `matched` before using `method`, and warn on miss rather than throwing. Other patches in the bundle then still apply.

`matchedInstructionIndices` returns where each `instructions(...)` predicate fired, in the order given, so splices can reference them positionally:

```kotlin
val constStringIdx = apiCallFingerprint.matchedInstructionIndices[0]
method.replaceInstruction(constStringIdx, /* ... */)
```

To iterate every match instead of the first:

```kotlin
for (match in apiCallFingerprint.findAll()) {
    val m = match.method
    // ...
}
```
