---
description: Hook the app's JavaScript functions in a Hermes bundle, and call your TypeScript from the hooks.
---

# Hermes JavaScript

React Native apps often ship their JavaScript as Hermes bytecode, in `assets/index.android.bundle`. Reseam opens that file in the base APK when a patch first uses it, links your extensions into it, and writes the patched bundle back when the run finishes. A bundle no patch changed is left as it was.

Bytecode version 98 is supported. A missing bundle, a file in another format, or another bytecode version fails the patch.

You hook the app's JavaScript functions the way you [change methods](7_changing_code.md), and call your own TypeScript or JavaScript from the hooks.

## Write an extension

Put your code in an extension module of its own, with one entry module:

```text
apps/example/extensions/features/
  src/main/js/index.ts
```

The entry is `index.ts`, `index.tsx`, `index.js`, or `index.jsx`. The build bundles it with the modules it imports by relative path, and compiles it with a pinned Hermes compiler. Kotlin and JavaScript extensions need separate modules.

Export functions that take values and return values:

```typescript
export function withFeature(flags: Record<string, boolean>, name: string) {
    return { ...flags, [name]: true };
}
```

The build generates a binding for each extension, from its exports. A patch module in `apps/example` gets `app.reseam.example.bindings.Features`, with one property per export.

The extension first runs when a hook first calls it. Its exports stay private: they add nothing to the app's global object.

## Find and hook

Declare the app function as a target, as you would a method, and hook it in `execute`:

```kotlin
import app.reseam.example.bindings.Features

private val featureFlags = function {
    name("getFeatureFlags")
    strings("FEATURE_FLAGS")
    paramCount(1)
}

val allFeatures = patch(ExamplePatchStrings.allFeatures) {
    execute {
        featureFlags.after {
            returnValue(Features.withFeature(result, string("search")))
        }
    }
}
```

Every constraint must match:

- `name` is the function's exact name, case included.
- `strings` matches strings the function's instructions reference, property names included.
- `paramCount` counts declared parameters, not counting `this`.

No match, or several, fails the patch. Like other targets, a function is searched for the first time a patch uses it.

| On a function | |
|---|---|
| `before { }` | at the start. Falling off the end calls the function with its arguments, as the block left them. |
| `after { }` | after it returns. `result` is the returned value. |
| `replace { }` | instead of the function. Falling off the end returns `undefined`. |

| In a hook | |
|---|---|
| `thisObject`, `param(i)`, `result` | the receiver, the arguments, and the returned value |
| `int(1)`, `bool(true)`, `string("x")`, `nullValue`, `undefinedValue` | constants |
| `Features.withFeature(args)` | call an export and use its result |
| `value.assign(v)` | overwrite an argument, the result, or a call's result |

Branches and returns work as they do for methods, but test as JavaScript does: truthiness, `== null`, and strict equality. [API: Hermes](reference_hermes.md) lists everything.

### How hooks combine

Hooks compose in the order patches add them: a later hook runs around the function as earlier hooks left it. `replace` stops earlier hooks from running. Targets still match the original app functions, so a hook never changes what another patch's target finds.

Some functions can't be hooked, and fail the patch: generators, async functions, class constructors, functions that use `new.target` or direct `eval`, and functions whose surrounding scopes Reseam can't work out. A hooked function can't be called with `new` afterwards, and may lose its debug information.

## Use the app's modules

React Native bundles keep their source modules in Metro, the bundle's module system. Your extension imports them with bare specifiers:

```typescript
import UserStore from "app:stores/UserStore.tsx";
import { cloneElement } from "react";
```

One patch declares which Metro module each specifier means, for the whole run. Patches whose extensions import app modules depend on it:

```kotlin
val appModules = patch {
    execute {
        hermes.modules {
            module("react", function { name("isValidElement"); paramCount(1) }.module)
            prefix("app:") { path -> function { strings(path); paramCount(7) }.module }
        }
    }
}
```

`FunctionTarget.module` is the Metro module whose factory creates the function, or the function itself when it is a factory. A function created outside a module, or in several, fails the patch. JSX compiles to the app's `react/jsx-runtime`, so declare that module when you use JSX.

Each import is read through Metro the first time your extension uses it. So an extension that runs early in the app's startup never sees a half-initialized module.

## Type checking

An app folder with a `package.json` is type checked before its extensions are bundled, as the task `typecheck-<app>`.

- `package.json` pins the type packages your extensions use, as `devDependencies`.
- `package-lock.json` holds their integrity hashes.
- `tsconfig.json` configures the check.

Only the packages' declaration files are installed, into the folder's `node_modules`.

A Kotlin extension in the same app that defines React Native modules gets declarations for them. A class with `@ReactModule(name = ...)` and `@ReactMethod` methods becomes a typed member of `NativeModules` from `react-native`, in the extension's `build/reseam/types`. Include `extensions/*/build/reseam/types` in `tsconfig.json`, and a call with a wrong method name or argument count fails the check.

## Gate on a setting

`alwaysReturn()`, `alwaysReturnNull()`, and `alwaysReturn(value)` replace a function's body with a constant result, as they do for methods.

[Gates](7_changing_code.md#gates) apply to Hermes targets too:

```kotlin
gate(AppSettings.allFeatures) {
    isStaff.alwaysReturn(true)
    featureFlags.after { returnValue(Features.withFeature(result, string("search"))) }
    renderRow.setArgument(0, "options.animate", false)
}
```

While the toggle is off, the function runs unchanged, so your extension never checks settings itself. `setArgument(index, path, value)` calls the function with one property of an argument replaced. The function receives copies of the objects along the path, so the caller's object doesn't change.

Hermes gates read each toggle once per process, through the [app runtime](11_settings.md#react-native-apps)'s React Native module. Your settings host adds it to the app with `addReactPackage`. Inside a Hermes hook, `whenEnabled` and `valueOf` read a toggle too.

Next: [Bundle projects](15_bundles.md).
