# Structural Matching

Reseam matches bytecode with structural queries, ranked candidates, and runtime bindings.

The main authoring entry points are:

- `ctx.findMethod { ... }`
- `ctx.findMethods { ... }`
- `ctx.findClass { ... }`
- `ctx.bind<T> { ... }`

These queries are index-backed for the duration of the patch run. Structural signals narrow the candidate set first, then `rankBy(...)` breaks ties on the remaining matches.

Common signals include:

- string constants
- literal values
- return type
- exact parameter list
- parameter presence
- containing class
- caller and callee relationships
- opcode presence
- class field shape

When a query fails, the engine reports:

- how many indexed candidates were considered
- which constraints were applied
- near misses and why they lost

Use [Queries And Bindings](3_2_queries.md) for the authoring surface and [Runtime surface](3_1_runtime.md) for the lower-level bytecode APIs.
