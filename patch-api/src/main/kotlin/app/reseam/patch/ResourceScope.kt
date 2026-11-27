// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.native.resAddAt
import app.reseam.patch.native.resAddFile
import app.reseam.patch.native.resAddFileData
import app.reseam.patch.native.resAddId
import app.reseam.patch.native.resAddIn
import app.reseam.patch.native.resAddRaw
import app.reseam.patch.native.resArrayGet
import app.reseam.patch.native.resArraySet
import app.reseam.patch.native.resArrayValuesGet
import app.reseam.patch.native.resArrayValuesSet
import app.reseam.patch.native.resAttrAdd
import app.reseam.patch.native.resComponentFor
import app.reseam.patch.native.resComponentForId
import app.reseam.patch.native.resComponentNames
import app.reseam.patch.native.resExists
import app.reseam.patch.native.resExistsIn
import app.reseam.patch.native.resFilePath
import app.reseam.patch.native.resFilePathById
import app.reseam.patch.native.resFilePaths
import app.reseam.patch.native.resFiles
import app.reseam.patch.native.resGetRaw
import app.reseam.patch.native.resGetString
import app.reseam.patch.native.resGetStringById
import app.reseam.patch.native.resId
import app.reseam.patch.native.resNames
import app.reseam.patch.native.resPoolAdd
import app.reseam.patch.native.resPoolFindRefs
import app.reseam.patch.native.resPoolGet
import app.reseam.patch.native.resPoolSet
import app.reseam.patch.native.resReferenceById
import app.reseam.patch.native.resReplaceEntry
import app.reseam.patch.native.resSetPackageName
import app.reseam.patch.native.resSetString
import app.reseam.patch.native.resSetStringById
import app.reseam.patch.native.resStringArraySet
import app.reseam.patch.native.resStyleItems
import app.reseam.patch.native.resStyleSet
import app.reseam.patch.native.resStyleSetIn
import app.reseam.patch.native.xmlOpen
import app.reseam.patch.types.ResourceRef
import app.reseam.patch.types.ResourceScalar
import app.reseam.patch.types.StyleItem

/**
 * Reads and edits compiled resources in the selected APK component. Missing names return `null` or
 * `false` from the corresponding lookup methods. An unreadable table or a malformed matching entry
 * throws with the parse error; a malformed unrelated entry does not prevent access to a valid
 * resource.
 */
class ResourceScope internal constructor(private val componentName: String? = null) {
    fun components(): List<String> = resComponentNames()

    fun component(name: String): ResourceScope = ResourceScope(name)

    /**
     * Renames the resource table's package. An app installed under a new package name needs this:
     * `Resources.getIdentifier(name, type, context.packageName)` matches the table's package name,
     * so every by-name lookup of the app's own resources fails until the table carries the new
     * name. Fails for a name over 127 UTF-16 units.
     */
    fun setPackageName(name: String) = resSetPackageName(componentName, name)

    fun owningComponent(resType: String, resName: String): String? =
        resComponentFor(resType, resName)

    fun owningComponent(resId: UInt): String? = resComponentForId(resId)

    fun id(resType: String, resName: String): UInt? = resId(componentName, resType, resName)

    fun exists(resType: String, resName: String): Boolean =
        resExists(componentName, resType, resName)

    fun names(resType: String): List<String> = resNames(componentName, resType)

    /**
     * Whether the resource has a value in exactly the configuration [qualifiers] names, `""` being
     * the default.
     */
    fun exists(resType: String, resName: String, qualifiers: String): Boolean =
        resExistsIn(componentName, resType, resName, qualifiers)

    fun getString(name: String): String? = resGetString(componentName, name)

    fun setString(name: String, value: String): Boolean = resSetString(componentName, name, value)

    /** [getString] by the id a manifest or XML attribute references. */
    fun getString(resId: UInt): String? = resGetStringById(componentName, resId)

    /** [setString] by resource id. */
    fun setString(resId: UInt, value: String): Boolean =
        resSetStringById(componentName, resId, value)

    fun add(resType: String, name: String, value: String): UInt? = add(resType, name, value, "")

    /**
     * Adds or replaces `resType/name` in the configuration [qualifiers] names, as [addFile] reads
     * them.
     */
    fun add(resType: String, name: String, value: String, qualifiers: String): UInt? =
        resAddIn(componentName, resType, name, value, qualifiers)

    /**
     * Adds or replaces `resType/name` with the fixed [resId], for code that refers to it by number,
     * such as a resource table built ahead of time. Fails when [resId] is not a `resType` id of the
     * app, another resource has it, or the name already has another id.
     */
    fun addAt(
        resId: UInt,
        resType: String,
        name: String,
        value: String,
        qualifiers: String = "",
    ): UInt =
        requireNotNull(resAddAt(componentName, resType, name, value, qualifiers, resId)) {
            "$resType/$name: $value is not a $resType value"
        }

    /**
     * Adds or replaces `attr/name`, accepting [formats] as `<attr format="...">` writes them, such
     * as `reference|color`. Styles can then set it and resources read it with `?attr/name`.
     */
    fun addAttr(name: String, formats: String): UInt = resAttrAdd(componentName, name, formats)

    fun addString(name: String, value: String): UInt? = add("string", name, value)

    fun addBool(name: String, value: Boolean): UInt? = add("bool", name, value.toString())

    fun addInteger(name: String, value: Int): UInt? = add("integer", name, value.toString())

    fun addColor(name: String, color: String): UInt? = add("color", name, color)

    fun addColor(name: String, color: String, qualifiers: String): UInt? =
        add("color", name, color, qualifiers)

    fun addDimen(name: String, dimen: String): UInt? = add("dimen", name, dimen)

    fun addId(name: String): UInt? = resAddId(componentName, name)

    fun addRaw(resType: String, name: String, dataType: UByte, data: UInt): UInt? =
        resAddRaw(componentName, resType, name, dataType, data)

    fun getRaw(resType: String, resName: String): Long? = resGetRaw(componentName, resType, resName)

    /**
     * The default configuration's APK path for `resType/resName`. Fails if the resource or its APK
     * entry is missing, or the value is not a file path.
     */
    fun path(resType: String, resName: String): String =
        resFilePath(componentName, resType, resName)

    fun files(resType: String, resName: String): List<ResourceFile> =
        resFiles(componentName, resType, resName).map { ResourceFile(it.qualifiers, it.path) }

    fun path(resId: UInt): String = resFilePathById(componentName, resId)

    /**
     * The reference resource XML writes for [resId], such as `@id/fullscreen_button` or
     * `@android:color/white`, or null when no resource has the id.
     */
    fun reference(resId: UInt): String? = resReferenceById(componentName, resId)

    /** [path] for every configuration that defines the entry, the default configuration first. */
    fun paths(resType: String, resName: String): List<String> =
        resFilePaths(componentName, resType, resName)

    /** The default configuration's file of `resType/resName`, opened as an XML document. */
    fun xml(resType: String, resName: String): XmlDocument {
        val component =
            componentName
                ?: owningComponent(resType, resName)
                ?: error("no component defines $resType/$resName")
        val path = resFilePath(component, resType, resName)
        return XmlDocument(
            xmlOpen(component, path) ?: error("failed to open $resType/$resName at $path")
        )
    }

    fun <T> editXml(resType: String, resName: String, block: XmlDocument.() -> T): T =
        xml(resType, resName).use(block)

    /**
     * Registers an APK entry as the file behind `resType/name` in the configuration [qualifiers]
     * names, written as in a `res/` directory name (`""`, `"pt-rBR"`, `"xxhdpi"`, `"night-v31"`).
     * Each configuration of one resource uses the same [name]. Fails when the APK has no entry at
     * [apkPath], or for a qualifier other than a language and region, a screen size, `night` or
     * `notnight`, a density and `vN`, or one out of that order.
     */
    fun addFile(resType: String, name: String, apkPath: String, qualifiers: String = ""): UInt =
        resAddFile(componentName, resType, name, apkPath, qualifiers)

    /**
     * Writes [data] to [apkPath] and registers it like [addFile]. XML is compiled, and each
     * `<aapt:attr>` in it becomes a resource of its own, `$name__N` of the same type next to
     * [apkPath], which the attribute it stands for then references, as aapt builds them.
     */
    fun addFile(
        resType: String,
        name: String,
        apkPath: String,
        data: ByteArray,
        qualifiers: String = "",
    ): UInt = resAddFileData(componentName, resType, name, apkPath, data, qualifiers)

    /**
     * Adds or replaces items in every configuration of `style/name`. Creating a style requires
     * [parent] and uses the default configuration. For an existing style, a supplied [parent]
     * replaces its parent.
     */
    fun style(name: String, parent: String? = null, block: StyleScope.() -> Unit): UInt =
        resStyleSet(componentName, name, parent, StyleScope().apply(block).items)

    /**
     * [style] for the one configuration [qualifiers] names, written as for [addFile] (`"night"`). A
     * configuration without the style starts from the default configuration's parent and items,
     * because the app reads the whole style from the configuration that matches.
     */
    fun styleIn(
        name: String,
        qualifiers: String,
        parent: String? = null,
        block: StyleScope.() -> Unit,
    ): UInt =
        resStyleSetIn(componentName, name, parent, StyleScope().apply(block).items, qualifiers)

    /**
     * The items of style [name] in the configuration [qualifiers] names, attribute to value, both
     * as resource XML writes them: `android:textColor` or an app attribute's bare name, and
     * `@color/name`, `?attr/name` or a literal. Null when that configuration has no such style.
     */
    fun styleItems(name: String, qualifiers: String = ""): Map<String, String>? =
        resStyleItems(componentName, name, qualifiers)?.associate { it.name to it.value }

    /**
     * The elements of `array/name` as text. Use [setStringArray] to preserve string-array values.
     */
    fun getArray(name: String): List<String> = resArrayGet(componentName, name)

    /**
     * Replaces the elements of `array/name`, in every configuration that defines it, or adds the
     * array in the default configuration. The count may change. Values are read the way attribute
     * values are.
     */
    fun setArray(name: String, values: List<String>): UInt =
        resArraySet(componentName, name, values)

    /**
     * Writes literal strings in every configuration, including numeric text and text starting with
     * `@`. A missing array is added in the default configuration.
     */
    fun setStringArray(name: String, values: List<String>): UInt =
        resStringArraySet(componentName, name, values)

    /**
     * The default array configuration without text conversion. Kinds and packed bits survive a
     * read/write round trip, including references, dimensions, colors and null. String data is an
     * index in this component's pool; use [poolGet] and [poolAdd] when copying between components.
     */
    fun getArrayValues(name: String): List<ResourceValue> =
        resArrayValuesGet(componentName, name).map { ResourceValue(it.kind, it.data) }

    /**
     * Replaces the elements of `array/name` in all its configurations without interpreting values
     * as text, keeping its id and configurations, or adds the array in the default configuration.
     * Invalid string indices fail.
     */
    fun setArrayValues(name: String, values: List<ResourceValue>): UInt =
        resArrayValuesSet(componentName, name, values.map { ResourceScalar(it.kind, it.data) })

    fun poolGet(index: UInt): String? = resPoolGet(componentName, index)

    fun poolSet(index: UInt, value: String) = resPoolSet(componentName, index, value)

    fun poolAdd(value: String): UInt? = resPoolAdd(componentName, value)

    fun poolFindRefs(stringIndex: UInt): List<ResourceRef> =
        resPoolFindRefs(componentName, stringIndex)

    fun replaceEntry(resId: UInt, newStringIndex: UInt) =
        resReplaceEntry(componentName, resId, newStringIndex)
}

/** The `<item>`s [ResourceScope.style] writes. */
class StyleScope internal constructor() {
    internal val items = mutableListOf<StyleItem>()

    /**
     * Sets `<item name="attr">value</item>`. `android:name` names a framework attribute and an
     * unprefixed name one the app declares, which is what a style item name means in resource XML.
     * An attribute that resolves to no id fails the patch, since the framework would ignore the
     * item.
     */
    operator fun set(attr: String, value: String) {
        items += StyleItem(attr, value)
    }
}

/** An Android resource scalar. [kind] is the Android type byte and [data] its unmodified bits. */
data class ResourceValue(val kind: UByte, val data: UInt)

data class ResourceFile(val qualifiers: String, val path: String)
