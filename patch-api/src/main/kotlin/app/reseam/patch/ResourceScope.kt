// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.native.resAdd
import app.reseam.patch.native.resAddFile
import app.reseam.patch.native.resAddFileData
import app.reseam.patch.native.resAddId
import app.reseam.patch.native.resAddRaw
import app.reseam.patch.native.resArrayGet
import app.reseam.patch.native.resArraySet
import app.reseam.patch.native.resArrayValuesGet
import app.reseam.patch.native.resArrayValuesSet
import app.reseam.patch.native.resComponentFor
import app.reseam.patch.native.resComponentForId
import app.reseam.patch.native.resComponentNames
import app.reseam.patch.native.resExists
import app.reseam.patch.native.resFilePath
import app.reseam.patch.native.resFilePaths
import app.reseam.patch.native.resGetRaw
import app.reseam.patch.native.resGetString
import app.reseam.patch.native.resId
import app.reseam.patch.native.resPoolAdd
import app.reseam.patch.native.resPoolFindRefs
import app.reseam.patch.native.resPoolGet
import app.reseam.patch.native.resPoolSet
import app.reseam.patch.native.resReplaceEntry
import app.reseam.patch.native.resSetPackageName
import app.reseam.patch.native.resSetString
import app.reseam.patch.native.resStringArraySet
import app.reseam.patch.native.resStyleSet
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

    fun getString(name: String): String? = resGetString(componentName, name)

    fun setString(name: String, value: String): Boolean = resSetString(componentName, name, value)

    fun add(resType: String, name: String, value: String): UInt? =
        resAdd(componentName, resType, name, value)

    fun addString(name: String, value: String): UInt? = add("string", name, value)

    fun addBool(name: String, value: Boolean): UInt? = add("bool", name, value.toString())

    fun addInteger(name: String, value: Int): UInt? = add("integer", name, value.toString())

    fun addColor(name: String, color: String): UInt? = add("color", name, color)

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
     * names, written as in a `res/` directory name (`""`, `"xxhdpi"`, `"anydpi-v26"`, `"night"`).
     * Each configuration of one resource uses the same [name]. Fails when the APK has no entry at
     * [apkPath] or a qualifier is not a density, `night`, `notnight` or `vN`.
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
     * The elements of `array/name` as text. Use [setStringArray] to preserve string-array values.
     */
    fun getArray(name: String): List<String> = resArrayGet(componentName, name)

    /**
     * Replaces the elements of `array/name`, in every configuration that defines it. The count may
     * change. Values are read the way attribute values are.
     */
    fun setArray(name: String, values: List<String>): UInt =
        resArraySet(componentName, name, values)

    /**
     * Writes literal strings in every configuration, including numeric text and text starting with
     * `@`.
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
     * Replaces an existing array in all its configurations without interpreting values as text.
     * Invalid string indices fail. Resource identity and configuration coverage are retained.
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
