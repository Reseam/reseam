---
description: Reach a value several obfuscated classes deep, and reuse the path in any patch.
---

# Reading obfuscated objects

Sometimes a patch holds an object and needs a value buried inside it. Say a feed post's video link is `post.attributes.videos[0].getUrl()`, and every class on that path has a meaningless name. A *binding* describes the path once, by structure. Any patch can then use it and get null-checked code that walks the path.

```kotlin
val post = bind("post") {
    fromField("feedPostField") {
        owner(onPostClicked.owner)
        nearestObjectReadBeforeString("post_clicked")
    }
    string("videoUrl") {
        member("attributes")
        listGetter("videos") {
            rankBy("callers followed by cast") { callSitesFollowedByCast(VIDEO) }
        }
        first()
        cast(VIDEO)
        callInterface(VIDEO, "getUrl", "()Ljava/lang/String;")
    }
}
```

- **The source** (`fromField`, `fromMethod`, or `fromClass`) finds the class the path starts from.
- **Members** (`string`, `objectValue`, `intValue`, `context`, `bind`) are named paths. Each step reads a field, calls a method, casts, or takes a list element.

Use it in a code block:

```kotlin
onPostClicked.before {
    val url = post.member("videoUrl", thisObject.field(post.sourceField))
    whenNotNull(url) { call(Downloads.offer, url) }
}
```

If any step on the path is null, the result is null (or zero), so check it with `whenNotNull`.

> [!WARNING]
> A binding finds the class, not where the object is kept. Your patch still has to read the object first, as `thisObject.field(post.sourceField)` does above.

The [reference](reference.md#bindings) lists every source and step.

Next: [Shipping your own code](11_extensions.md).
