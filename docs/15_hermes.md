---
description: Link JavaScript extensions into a Hermes bundle and wrap the app's functions.
---

# Hermes JavaScript

React Native apps often ship JavaScript as Hermes bytecode in `assets/index.android.bundle`. Inside `execute { }`, `hermes` opens that bundle in the base APK. Reseam links your JavaScript into it and writes the patched bundle when the run finishes.

Version 98 is supported. A missing bundle, another file format, or another bytecode version fails the patch.

## Write an extension

Put JavaScript in an extension module:

```text
apps/discord/extensions/emotes/
  src/main/js/index.js
```

Assign function expressions to `exports` at the top level:

```javascript
exports.allow = function (original, ...args) {
    return true;
};
```

The build compiles every `.js` file in `src/main/js`, in relative path order, inside one strict function with its own `exports` object. It fetches the pinned compiler for each supported Hermes version and puts the compiled modules in the bundle. Java and JavaScript extensions need separate modules.

Keep export assignments unconditional and use literal property names. Reseam checks that a declared export is a callable property of the returned object. Dynamic declarations it cannot establish fail on use.

The module initializes once before the app's global code. Strings, regexps, object and array literals, bigints, and closures can be used normally. App globals may not exist during initialization. Exports stay in a private lexical environment; they do not add properties to the app's global object.

## Find and wrap

Declare the module and the exports a patch uses:

```kotlin
object Emotes : ExtJsModule("discord-emotes") {
    val allow = export("allow")
}
```

The artifact name follows the workspace convention: `<app>-<extension>`, or `<name>` for a shared module.

Then find a function inside `execute`:

```kotlin
execute {
    hermes.function {
        name("canUseAnimatedEmojis")
        strings("ANIMATED_EMOJIS")
        paramCount(1)
    }.wrap(Emotes.allow)
}
```

Every constraint must match. `name` is exact and case-sensitive; `strings` matches instruction references, including property names; `paramCount` excludes `this`. Zero or several matches fail the patch. Function handles are valid for that patch run only.

`wrap` calls your export as `export(original, ...arguments)` with the function's receiver, and returns your export's result. `original` calls the previous wrap, or the unchanged body for the first wrap, with the original captured environment. It is bound to the receiver, so `original(...args)` preserves `this` too.

Repeated wraps compose in the order patches apply them. The last wrap runs first and receives the previous wrap as `original`; the innermost `original` calls the unchanged app body. Exports from different modules can wrap the same function. Lookups continue to match the original app functions.

## Before, after, replace

These are JavaScript around the same primitive:

```javascript
exports.before = function (original, ...args) {
    recordRequest(args);
    return original(...args);
};

exports.after = function (original, ...args) {
    return adjustResult(original(...args));
};

exports.replace = function (original, ...args) {
    return replacementValue(args);
};
```

An export can change arguments, call `original` several times, skip it, or catch exceptions from it. Exceptions it does not catch propagate to the app's caller.

Generators, async functions, class constructors, functions using `new.target` or direct `eval`, and environment chains Reseam cannot establish fail explicitly. An ordinary wrapped function cannot be called with `new` afterwards. Edited functions may lose their debug information.

## Another bundle path

```kotlin
val code = hermes.bundle("assets/application.hbc")
val version = code.version
```

A run can open one Hermes bundle. Choose the path before using the default scope. The engine writes it stored, without ZIP compression.

See the [reference](reference.md#hermes) for the authoring API.
