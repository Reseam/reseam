# Queries And Bindings

Structural queries and bindings are the primary authoring surface for bytecode discovery and runtime extraction:

```kotlin
val feedClickHandler = ctx.findMethod(debug = "feedClickHandler") {
    strings("click_media_option", "MediaOptionsOverflowHelper")
    returnType("V")
    parameterTypes("Lcom/example/MenuOption;")
}

val feedClickListener = ctx.findMethod(debug = "feedClickListener") {
    returnType("V")
    parameterTypes("Landroid/view/View;")
    calls(feedClickHandler)
}
```

Normal query APIs are index-backed and memoized for the patch run:

- `ctx.findMethod { ... }`
- `ctx.findMethods { ... }`
- `ctx.findClass { ... }`
- `ctx.classHandle("Lcom/example/Type;")`
- `ctx.bind<T> { ... }`

The only explicit full scan is:

- `ctx.scanMethod { ... }`

## Method Queries

Available structural signals:

- `strings(...)`
- `literals(...)`
- `returnType(...)`
- `parameterTypes(...)`
- `hasParameter(...)`
- `inClass(...)`
- `sameAs(...)`
- `calls(...)`
- `calledBy(...)`
- `hasOpcode(...)`
- `rankBy(label) { ... }`

Every query accepts an optional `debug` label. Failed matches expose diagnostics through `ctx.explain(handle)` and through the error text emitted by `findMethod` / `findClass`.

Use `ctx.classHandle(descriptor)` when another query or binding has already produced a concrete descriptor and you need a `ClassHandle` for `inClass(...)` or `fromClass(...)`.

## Bindings

`Binding<T>` is the runtime extraction abstraction for obfuscated object graphs.

```kotlin
val media = ctx.bind<RuntimeMedia>(debug = "media") {
    fromField(debug = "feedMediaField") {
        owner(feedClickHandler.classDescriptor)
        nearestObjectReadBeforeString("click_media_option")
    }

    string("imageUrl") {
        field(type = "Lcom/instagram/model/mediasize/ExtendedImageUrl;")
        callVirtual(
            "Lcom/instagram/model/mediasize/ExtendedImageUrl;",
            "getUrl",
            "()Ljava/lang/String;",
        )
    }
}
```

Binding members are named by intent:

- `string(...)`
- `objectValue(...)`
- `context(...)`
- `bind(...)`
- `list(...)`

Use them from mutations and stubs with:

- `binding.of(raw)`
- `binding.member(name, on)`
- `ctx.bindStub(owner) { ... }`

When a binding is applied to an `Object`-typed stub parameter, the engine inserts the binding root `check-cast` before the first field or call in the path. Patch authors should pass the raw stub value and let the binding lower it.

## Field Locators

Field locators operate in two different modes:

- `fromField { owner(X); firstObjectRead() }` scans methods on `X`, but it only matches `IGET_OBJECT` instructions whose field is declared on `X`.
- `fromField { owner(X); firstObjectReadAnyOwner() }` scans methods on `X` and accepts the first `IGET_OBJECT` regardless of which class declares the field.
- `nearestObjectReadBeforeString("...")` also stays owner-scoped: it looks for an `IGET_OBJECT` before the anchor string whose field is declared on the selected owner.

If the intent is method-local instruction order, use a method anchor instead:

```kotlin
val method = ctx.findMethod(debug = "carrier") { ... }

val binding = ctx.bind<RuntimeThing>(debug = "thing") {
    fromMethod(debug = "carrier") { sameAs(method) }
    raw {
        firstFieldRead()
    }
}
```

## Mutation

Preferred mutation entry points are:

- `methodHandle.before { ... }`
- `methodHandle.after { ... }`
- `methodHandle.replace { ... }`
- `methodHandle.prependWhen(setting) { ... }`
- `methodHandle.returnTrueWhen(setting)`
- `methodHandle.returnFalseWhen(setting)`
- `methodHandle.returnNullWhen(setting)`
- `methodHandle.skipWhen(setting)`

Inside `CodeScope`, the engine owns scratch allocation, invoke widening, and `outs_size` growth.
