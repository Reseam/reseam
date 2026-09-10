# Reading obfuscated objects

Some patches need a value out of an object whose class, fields, and getters are all renamed: the image URL inside Instagram's media object, for example. A binding describes how to find that class and the path from it to each value (a field read, a call, a cast). Once declared, a patch applies it to any value of that class inside a code block and gets the member out, null-checked along the way.

A binding resolves once per patch.

```kotlin
import app.reseam.patch.bind

interface RuntimeMedia

val media = bind<RuntimeMedia>("media") {
    fromField("feedMediaField") {
        owner(feedClickHandler.owner)
        nearestObjectReadBeforeString("click_media_option")
    }
    string("imageUrl") {
        field(EXTENDED_IMAGE_URL)
        callVirtual(EXTENDED_IMAGE_URL, "getUrl", "()Ljava/lang/String;")
    }
    string("videoUrl") {
        member("dict")
        listGetter("video_versions") {
            rankBy("callers followed by cast") { callSitesFollowedByCast(VIDEO_VERSION_INTF) }
        }
        first()
        cast(VIDEO_VERSION_INTF)
        callInterface(VIDEO_VERSION_INTF, "getUrl", "()Ljava/lang/String;")
    }
}
```

The type parameter is a marker for readers. `sourceType` is the root descriptor once resolved; `sourceField` is the field it was located through.

## Sources

- `fromField("label") { }`: locate a field with `owner(type)`, then `firstObjectRead()`, `firstObjectReadAnyOwner()`, `nearestObjectReadBeforeString(value)`, or `rankBy` with `requireScoreAtLeast(score)`.
- `fromMethod(target)`: anchor paths in a method's instructions.
- `fromClass(target)`: start from a known class.

`raw { }` is the path from the input value to the bound object. Inside it, `sourceType` is the source's type; after it, the raw path's result.

## Members

`objectValue(name) { }`, `string(name) { }`, `context(name) { }`, `intValue(name) { }` declare members by the kind of value they produce. `bind(name, otherBinding) { }` declares a member whose value is another binding's root when the path does not determine a type itself.

Steps:

| Step | Meaning |
|---|---|
| `self()`, `param(i)` | Start from the anchor method's receiver or parameter. |
| `member(name)` | Start from another member's path. |
| `field(type)`, `field(name) { locator }`, `field(fieldTarget)` | Read a field. |
| `instanceField(type)`, `instanceField(listOf(a, b))` | Read the one instance field of a type, or the first present of several. |
| `objectSlots().firstInstanceOf(type)` | Probe `Object`-typed fields at runtime for the first instance of a type. |
| `firstFieldRead()`, `nextFieldRead(owner)` | Follow field reads in the anchor method. |
| `nextInterfaceCall(returning, returningObject)` | Follow an interface call in the anchor method. |
| `callVirtual(owner, name, proto)`, `callInterface(...)` | Call a method. |
| `listGetter(name) { rankBy }` | Pick a zero-argument `List` getter on the current type by rank. |
| `first()`, `last()` | An element of a `List`. |
| `cast(type)` | Check-cast. |

Every step is null-checked. A null anywhere makes the whole path evaluate to null or zero.

## Applying

```kotlin
MediaRefs.photoUrl.implement { returnValue(media.member("imageUrl", param(0))) }

feedClickHandler.before {
    val handler = thisObject
    val current = carouselState.member("currentIndex", handler.fieldOfType(carouselStateClass.descriptor))
}
```

`binding.of(value)` applies the raw path. `binding.member(name, value)` applies a member path. The input must be statically assignable to the root type; an `Object`-typed input is cast implicitly, anything else must be cast first.

Next: [Shipping your own code](8_extensions.md).
