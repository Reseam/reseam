# Reading obfuscated objects

Some patches hold an object and need a value from it that sits several renamed classes deep: the video URL of a feed post is `post.attributes.videos[0].getUrl()`, with every class on the way obfuscated. A binding describes that path once, structurally; any patch then applies it to a value inside a code block and gets the null-checked chain emitted.

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

onPostClicked.before {
    call(Ext.onClick, post.member("videoUrl", thisObject.field(post.sourceField)))
}

PostRefs.videoUrl.implement { returnValue(post.member("videoUrl", param(0))) }
```

The source (`fromField`, `fromMethod`, `fromClass`) finds the root class. Members (`string`, `objectValue`, `intValue`, `context`, `bind`) are named paths of steps: field reads by type or by locator, calls, casts, list element, another member as a prefix. The label names the binding in reports and logs. `raw { }` is the path from the input value to the root itself.

`binding.of(value)` applies the raw path; `binding.member(name, value)` a member. The input must be assignable to the root type; an `Object`-typed input is cast implicitly. A null anywhere on the path makes the result null or zero. Steps and sources are listed in the [reference](12_reference.md#bindings).

> [!WARNING]
> A binding knows the class, not where the object is held. The patch still reads it: `thisObject.field(post.sourceField)`. A member that reaches a null yields null rather than crashing, so handle it with `whenNotNull`.

Next: [Shipping your own code](9_extensions.md).
