---
description: Link JavaScript extensions into a Hermes bundle and wrap the app's functions.
---

# Hermes JavaScript

React Native apps often ship JavaScript as Hermes bytecode in `assets/index.android.bundle`. Reseam opens that bundle in the base APK when a patch first uses it, links your JavaScript into it, and writes the patched bundle stored, without ZIP compression, when the run finishes.

Version 98 is supported. A missing bundle, another file format, or another bytecode version fails the patch.

## Write an extension

Put JavaScript in an extension module:

```text
apps/discord/extensions/emotes/
  src/main/js/index.js
```

Assign function expressions to `exports` at the top level:

```javascript
exports.canUseAnimatedEmojis = function (original, user) {
    return true;
};
```

The build compiles every `.js` file in `src/main/js`, in relative path order, inside one strict function with its own `exports` object. It fetches the pinned compiler for each supported Hermes version and puts the compiled modules in the bundle. Java and JavaScript extensions need separate modules.

Keep export assignments unconditional and use literal property names. Reseam checks that a declared export is a callable property of the returned object. Dynamic declarations it cannot establish fail on use.

The module initializes once before the app's global code. Strings, regexps, object and array literals, bigints, and closures can be used normally. App globals may not exist during initialization. Exports stay in a private lexical environment; they do not add properties to the app's global object.

## Find and wrap

Declare the module and the exports a patch uses, named after the app functions they wrap:

```kotlin
object Emotes : ExtJsModule("discord-emotes") {
    val canUseAnimatedEmojis = export("canUseAnimatedEmojis")
}
```

The artifact name follows the workspace convention: `<app>-<extension>`, or `<name>` for a shared module.

Declare the app function as a target, like a DEX `method { }`, and wrap it in `execute`:

```kotlin
val animatedEmojis = patch("Animated emojis") {
    execute {
        canUseAnimatedEmojis.wrap(Emotes.canUseAnimatedEmojis)
    }
}

private val canUseAnimatedEmojis = function {
    name("canUseAnimatedEmojis")
    strings("ANIMATED_EMOJIS")
    paramCount(1)
}
```

Every constraint must match. `name` is exact and case-sensitive; `strings` matches instruction references, including property names; `paramCount` excludes `this`. Zero or several matches fail the patch. A target resolves when a patch first uses it.

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

## Gate on a setting

The [settings](5_settings.md) gates also apply to Hermes targets:

```kotlin
isStaff.returnTrueWhen(DiscordSettings.developerMenu)
canUseAnimatedEmojis.wrapWhen(DiscordSettings.animatedEmojis, Emotes.canUseAnimatedEmojis)
```

`skipWhen` returns `undefined`, and `returnNullWhen`, `returnTrueWhen` and `returnFalseWhen` return that value, when the toggle is on; otherwise they call the app function. `wrapWhen` runs your export when the toggle is on and the unchanged function otherwise, so the export itself does not check settings. `setArgumentWhen(setting, index, path, value)` calls the function with one property of an argument replaced, such as `setArgumentWhen(setting, 0, "options.animate", false)`; the objects along the path are copied, not changed. Each toggle is read once per process, through the `ReseamSettings` React Native module that the app's settings host registers.

See the [reference](reference.md#hermes) for the authoring API.
