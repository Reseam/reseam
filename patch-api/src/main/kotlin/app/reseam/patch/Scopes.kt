// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.dex.DexClass
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.parameterTypes
import app.reseam.patch.dex.returnType
import app.reseam.patch.native.MethodRef
import app.reseam.patch.native.ResourceRef
import app.reseam.patch.native.StyleItem
import app.reseam.patch.native.componentNames
import app.reseam.patch.native.fileCopy
import app.reseam.patch.native.fileDelete
import app.reseam.patch.native.fileInject
import app.reseam.patch.native.fileList
import app.reseam.patch.native.fileRead
import app.reseam.patch.native.fileSigners
import app.reseam.patch.native.fileSource
import app.reseam.patch.native.findInstructionsByString
import app.reseam.patch.native.findInstructionsByStringContains
import app.reseam.patch.native.logDebug
import app.reseam.patch.native.logInfo
import app.reseam.patch.native.logWarn
import app.reseam.patch.native.manifestAddActivityAlias
import app.reseam.patch.native.manifestAddIntentFilter
import app.reseam.patch.native.manifestAddPermission
import app.reseam.patch.native.manifestCopyIntentFilters
import app.reseam.patch.native.manifestGetDocument
import app.reseam.patch.native.manifestMinSdkVersion
import app.reseam.patch.native.manifestPackageName
import app.reseam.patch.native.manifestSetActivityConfigChanges
import app.reseam.patch.native.manifestSetAttributeInt
import app.reseam.patch.native.manifestSetAttributeString
import app.reseam.patch.native.manifestSetMinSdk
import app.reseam.patch.native.manifestSetVersionCode
import app.reseam.patch.native.manifestSetVersionName
import app.reseam.patch.native.manifestSplitName
import app.reseam.patch.native.manifestVersionCode
import app.reseam.patch.native.manifestVersionName
import app.reseam.patch.native.resAdd
import app.reseam.patch.native.resAddFile
import app.reseam.patch.native.resAddFileData
import app.reseam.patch.native.resAddId
import app.reseam.patch.native.resAddRaw
import app.reseam.patch.native.resArrayGet
import app.reseam.patch.native.resArraySet
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
import app.reseam.patch.native.xmlAdopt
import app.reseam.patch.native.xmlAppendChild
import app.reseam.patch.native.xmlChildren
import app.reseam.patch.native.xmlCloneElement
import app.reseam.patch.native.xmlClose
import app.reseam.patch.native.xmlCompile
import app.reseam.patch.native.xmlCreateElement
import app.reseam.patch.native.xmlDeclareNamespace
import app.reseam.patch.native.xmlFindByAttribute
import app.reseam.patch.native.xmlFindByTag
import app.reseam.patch.native.xmlGetAttribute
import app.reseam.patch.native.xmlInsertBefore
import app.reseam.patch.native.xmlOpen
import app.reseam.patch.native.xmlParent
import app.reseam.patch.native.xmlRemoveAttribute
import app.reseam.patch.native.xmlRemoveElement
import app.reseam.patch.native.xmlRoot
import app.reseam.patch.native.xmlSetAttribute
import app.reseam.patch.native.xmlSetAttributeRef
import app.reseam.patch.native.xmlTagName

class PatchLogger internal constructor() {
    fun info(message: String) = logInfo(message)
    fun warn(message: String) = logWarn(message)
    fun debug(message: String) = logDebug(message)
}

class BytecodeScope internal constructor() {
    val classes: List<DexClass>
        get() = ActiveRuntime.current.index.allClasses

    fun findClass(name: String): DexClass? = ActiveRuntime.current.index.classFor(descriptor(name))

    fun classesExtending(type: String): List<DexClass> {
        val wanted = descriptor(type)
        return classes.filter { ActiveRuntime.current.index.classExtends(it, wanted) }
    }

    /** Rewrites every `const-string` equal to `old` in the app; returns how many constants changed. */
    fun replaceAllStrings(old: String, new: String): Int =
        findInstructionsByString(old)
            .map { Method(it.method) }
            .distinctBy { it.handle }
            .sumOf { it.replaceAllStrings(old, new) }

    /**
     * Rewrites every `const-string` containing [substring] through [transform], which returns the
     * replacement or null to leave that constant as it is. Returns how many constants changed.
     *
     * Seeded from the string index, so only methods holding a matching constant are visited. Use it
     * when a value is embedded in larger constants rather than stored on its own, such as an
     * authority inside a `content://` URI; [replaceAllStrings] covers the whole-string case.
     */
    fun replaceStringsContaining(substring: String, transform: (String) -> String?): Int =
        findInstructionsByStringContains(substring)
            .mapNotNull { Method(it.method).stringRef(it.index.toInt()) }
            .distinct()
            .sumOf { old -> transform(old)?.takeIf { it != old }?.let { replaceAllStrings(old, it) } ?: 0 }

    /**
     * Every call to `from` in the app becomes a call to the static `to`, receiver first for
     * instance methods. Calls through `super`, constructor calls, and calls from extensions are
     * left alone, so `to` can call `from`. Returns how many call sites changed.
     */
    fun redirectCalls(from: MethodRef, to: ExtMethod): Int {
        val callers = app.reseam.patch.methods("callers of ${from.descriptor}") {
            calls(from)
        }
        return callers.points("redirect ${from.descriptor}") {
            invoke(*redirectableInvokes.toTypedArray()) {
                owner(from.definingClass)
                name(from.name)
                params(*from.parameterTypes.toTypedArray())
                returns(from.returnType)
            }
        }.all.redirectTo(to)
    }

    /**
     * [redirectCalls] with `from` derived from `to`: `to` takes the same parameters as `owner.name`,
     * preceded by the receiver when `owner.name` is an instance method, and returns the same type.
     */
    fun redirectCalls(owner: String, name: String, to: ExtMethod): Int {
        val ownerDesc = descriptor(owner)
        val params = to.ref.parameterTypes
        val fromParams = if (params.firstOrNull() == ownerDesc) params.drop(1) else params
        return redirectCalls(MethodRef(ownerDesc, name, proto(to.ref.returnType, *fromParams.toTypedArray())), to)
    }
}

class FileScope internal constructor(private val componentName: String? = null) {
    fun components(): List<String> = componentNames()
    fun component(name: String): FileScope = FileScope(name)

    fun list(): List<String> = fileList(componentName)
    fun read(path: String): ByteArray? = fileRead(componentName, path)
    /** The original, unmodified bytes of this component's APK, signing block included. */
    fun source(): ByteArray? = fileSource(componentName)
    /** The original signer certificates, DER-encoded X.509: v3 signers when present, otherwise v2; empty when unsigned or v1-only. */
    fun signers(): List<ByteArray> = fileSigners(componentName)
    fun write(path: String, data: ByteArray) = fileInject(componentName, path, data, false)
    fun writeStored(path: String, data: ByteArray) = fileInject(componentName, path, data, true)
    fun delete(path: String) = fileDelete(componentName, path)
    /** Copies a file shipped in the bundle into the APK. */
    fun copy(bundlePath: String, apkPath: String) = fileCopy(componentName, bundlePath, apkPath)

    fun xml(path: String): XmlDocument = XmlDocument(xmlOpen(componentName, path) ?: error("failed to open XML document: $path"))
    fun <T> editXml(path: String, block: XmlDocument.() -> T): T = xml(path).use(block)
}

/**
 * Reads and edits compiled resources in the selected APK component.
 * Missing names return `null` or `false` from the corresponding lookup methods.
 * An unreadable table or a malformed matching entry throws with the parse error;
 * a malformed unrelated entry does not prevent access to a valid resource.
 */
class ResourceScope internal constructor(private val componentName: String? = null) {
    fun components(): List<String> = resComponentNames()
    fun component(name: String): ResourceScope = ResourceScope(name)

    /**
     * Renames the resource table's package. An app installed under a new package name
     * needs this: `Resources.getIdentifier(name, type, context.packageName)` matches the
     * table's package name, so every by-name lookup of the app's own resources fails
     * until the table carries the new name. Fails for a name over 127 UTF-16 units.
     */
    fun setPackageName(name: String) = resSetPackageName(componentName, name)

    fun owningComponent(resType: String, resName: String): String? = resComponentFor(resType, resName)
    fun owningComponent(resId: UInt): String? = resComponentForId(resId)

    fun id(resType: String, resName: String): UInt? = resId(componentName, resType, resName)
    fun exists(resType: String, resName: String): Boolean = resExists(componentName, resType, resName)
    fun getString(name: String): String? = resGetString(componentName, name)
    fun setString(name: String, value: String): Boolean = resSetString(componentName, name, value)

    fun add(resType: String, name: String, value: String): UInt? = resAdd(componentName, resType, name, value)
    fun addString(name: String, value: String): UInt? = add("string", name, value)
    fun addBool(name: String, value: Boolean): UInt? = add("bool", name, value.toString())
    fun addInteger(name: String, value: Int): UInt? = add("integer", name, value.toString())
    fun addColor(name: String, color: String): UInt? = add("color", name, color)
    fun addDimen(name: String, dimen: String): UInt? = add("dimen", name, dimen)
    fun addId(name: String): UInt? = resAddId(componentName, name)
    fun addRaw(resType: String, name: String, dataType: UByte, data: UInt): UInt? = resAddRaw(componentName, resType, name, dataType, data)
    fun getRaw(resType: String, resName: String): Long? = resGetRaw(componentName, resType, resName)

    /**
     * The default configuration's APK path for `resType/resName`.
     * Fails if the resource or its APK entry is missing, or the value is not a file path.
     */
    fun path(resType: String, resName: String): String = resFilePath(componentName, resType, resName)

    /** [path] for every configuration that defines the entry, the default configuration first. */
    fun paths(resType: String, resName: String): List<String> = resFilePaths(componentName, resType, resName)

    /** The default configuration's file of `resType/resName`, opened as an XML document. */
    fun xml(resType: String, resName: String): XmlDocument {
        val component = componentName ?: owningComponent(resType, resName)
            ?: error("no component defines $resType/$resName")
        val path = resFilePath(component, resType, resName)
        return XmlDocument(xmlOpen(component, path) ?: error("failed to open $resType/$resName at $path"))
    }

    fun <T> editXml(resType: String, resName: String, block: XmlDocument.() -> T): T = xml(resType, resName).use(block)

    /**
     * Registers an APK entry as the file behind `resType/name` in the
     * configuration [qualifiers] names, written as in a `res/` directory name
     * (`""`, `"xxhdpi"`, `"anydpi-v26"`, `"night"`). Each configuration of one
     * resource uses the same [name]. Fails when the APK has
     * no entry at [apkPath] or a qualifier is not a density, `night`,
     * `notnight` or `vN`.
     */
    fun addFile(resType: String, name: String, apkPath: String, qualifiers: String = ""): UInt =
        resAddFile(componentName, resType, name, apkPath, qualifiers)

    /**
     * Writes [data] to [apkPath] and registers it like [addFile]. XML is compiled, and each
     * `<aapt:attr>` in it becomes a resource of its own, `$name__N` of the same type next to
     * [apkPath], which the attribute it stands for then references, as aapt builds them.
     */
    fun addFile(resType: String, name: String, apkPath: String, data: ByteArray, qualifiers: String = ""): UInt =
        resAddFileData(componentName, resType, name, apkPath, data, qualifiers)

    /**
     * Adds or replaces items in every configuration of `style/name`.
     * Creating a style requires [parent] and uses the default configuration.
     * For an existing style, a supplied [parent] replaces its parent.
     */
    fun style(name: String, parent: String? = null, block: StyleScope.() -> Unit): UInt =
        resStyleSet(componentName, name, parent, StyleScope().apply(block).items)

    /** The elements of `array/name` as text. Use [setStringArray] to preserve string-array values. */
    fun getArray(name: String): List<String> = resArrayGet(componentName, name)

    /**
     * Replaces the elements of `array/name`, in every configuration that defines
     * it. The count may change. Values are read the way attribute values are.
     */
    fun setArray(name: String, values: List<String>): UInt = resArraySet(componentName, name, values)

    /** Writes literal strings in every configuration, including numeric text and text starting with `@`. */
    fun setStringArray(name: String, values: List<String>): UInt = resStringArraySet(componentName, name, values)

    fun poolGet(index: UInt): String? = resPoolGet(componentName, index)
    fun poolSet(index: UInt, value: String) = resPoolSet(componentName, index, value)
    fun poolAdd(value: String): UInt? = resPoolAdd(componentName, value)
    fun poolFindRefs(stringIndex: UInt): List<ResourceRef> = resPoolFindRefs(componentName, stringIndex)
    fun replaceEntry(resId: UInt, newStringIndex: UInt) = resReplaceEntry(componentName, resId, newStringIndex)
}

/** The `<item>`s [ResourceScope.style] writes. */
class StyleScope internal constructor() {
    internal val items = mutableListOf<StyleItem>()

    /**
     * Sets `<item name="attr">value</item>`. `android:name` names a framework
     * attribute and an unprefixed name one the app declares, which is what a
     * style item name means in resource XML. An attribute that resolves to no
     * id fails the patch, since the framework would ignore the item.
     */
    operator fun set(attr: String, value: String) {
        items += StyleItem(attr, value)
    }
}

class ManifestScope internal constructor(private val componentName: String? = null) {
    fun components(): List<String> = componentNames()
    fun component(name: String): ManifestScope = ManifestScope(name)

    val packageName: String? get() = manifestPackageName(componentName)
    val versionCode: UInt? get() = manifestVersionCode(componentName)
    val versionName: String? get() = manifestVersionName(componentName)
    val minSdkVersion: UInt? get() = manifestMinSdkVersion(componentName)
    val splitName: String? get() = manifestSplitName(componentName)

    /** The fully qualified `<application android:name>`, or null when the app uses the platform default. */
    val applicationClass: String?
        get() = edit {
            val name = findByTag("application").firstOrNull()?.get("android:name") ?: return@edit null
            val pkg = packageName ?: return@edit name
            when {
                name.startsWith(".") -> pkg + name
                '.' !in name -> "$pkg.$name"
                else -> name
            }
        }

    fun setVersionCode(code: UInt) = manifestSetVersionCode(componentName, code)
    fun setVersionName(name: String) = manifestSetVersionName(componentName, name)
    fun setMinSdk(sdk: UInt) = manifestSetMinSdk(componentName, sdk)
    fun addPermission(permission: String) = manifestAddPermission(componentName, permission)
    fun setAttributeInt(elementName: String, attrName: String, value: Int) = manifestSetAttributeInt(componentName, elementName, attrName, value)
    /**
     * Sets an `android:` attribute of the first [elementName]. `@type/name` and
     * `?attr` become references to the base resource table and the attribute's
     * enum and flag names (`singleTask`, `orientation|screenSize`) become its
     * values, as aapt writes them; other text stays a string, and a leading
     * `\\@` or `\\?` keeps the character.
     */
    fun setAttributeString(elementName: String, attrName: String, value: String) = manifestSetAttributeString(componentName, elementName, attrName, value)
    fun setActivityConfigChanges(activityName: String, configChanges: String) = manifestSetActivityConfigChanges(componentName, activityName, configChanges)
    fun addIntentFilter(activityName: String, action: String? = null, category: String? = null, mimeType: String? = null) =
        manifestAddIntentFilter(componentName, activityName, action, category, mimeType)
    /**
     * Appends an `<activity-alias>` for [targetActivity] at the end of
     * `<application>`, after the activity it names as Android requires. [label]
     * is read like [setAttributeString] values, so `@string/name` is a reference.
     */
    fun addActivityAlias(targetActivity: String, aliasName: String, enabled: Boolean = true, label: String? = null) =
        manifestAddActivityAlias(componentName, targetActivity, aliasName, enabled, label)
    /** Copies every intent filter of [fromActivity] to [toActivity]; either may be an `<activity-alias>`. */
    fun copyIntentFilters(fromActivity: String, toActivity: String) = manifestCopyIntentFilters(componentName, fromActivity, toActivity)

    /** Declares an activity, unexported, unless the manifest already has it. `block` sets further attributes. */
    fun addActivity(name: String, block: XmlElement.() -> Unit = {}) = edit {
        val application = findByTag("application").firstOrNull() ?: error("manifest has no <application> element")
        if (findByAttribute("android:name", name).isNotEmpty()) return@edit
        application.appendChild(
            createElement("activity").apply {
                this["android:name"] = name
                this["android:exported"] = "false"
                block()
            },
        )
    }

    fun document(): XmlDocument = XmlDocument(manifestGetDocument(componentName) ?: error("manifest not available"))
    fun <T> edit(block: XmlDocument.() -> T): T = document().use(block)
}

class XmlDocument(val handle: UInt) : AutoCloseable {
    val root: XmlElement get() = XmlElement(handle, xmlRoot(handle))
    fun findByTag(tag: String): List<XmlElement> = xmlFindByTag(handle, tag).map { XmlElement(handle, it) }
    fun findByAttribute(name: String, value: String): List<XmlElement> = xmlFindByAttribute(handle, name, value).map { XmlElement(handle, it) }
    fun createElement(tag: String): XmlElement = XmlElement(handle, xmlCreateElement(handle, tag))

    /**
     * Declares [prefix] for [uri] unless the document already declares it.
     * Namespaces wrap the whole document, so this moves the index of every
     * element resolved before the call, like any other structural edit.
     */
    fun declareNamespace(prefix: String, uri: String) = xmlDeclareNamespace(handle, prefix, uri)

    /**
     * A detached deep copy of [element], which belongs to another document, in
     * this document's strings and namespaces. Attach it with [XmlElement.appendChild]
     * or [XmlElement.insertBefore]. Every attribute is rebound to the resource id
     * this document resolves it by, and one that resolves to none fails the
     * patch; declare a namespace the source uses and this document lacks with
     * [declareNamespace] first.
     */
    fun adopt(element: XmlElement): XmlElement = XmlElement(handle, xmlAdopt(handle, element.doc, element.handle))

    override fun close() = xmlClose(handle)

    companion object {
        /**
         * XML text compiled into a document of its own, resolving `@type/name`
         * references and attribute ids against the app's resource table. It is
         * backed by no APK entry, so closing it discards it.
         */
        fun compile(text: String): XmlDocument = XmlDocument(xmlCompile(text))
    }
}

class XmlElement(val doc: UInt, val handle: UInt) {
    val tag: String get() = xmlTagName(doc, handle)
    val parent: XmlElement? get() = xmlParent(doc, handle)?.let { XmlElement(doc, it) }
    val children: List<XmlElement> get() = xmlChildren(doc, handle).map { XmlElement(doc, it) }

    operator fun get(attr: String): String? = xmlGetAttribute(doc, handle, attr)

    /**
     * Sets `attr`, which is `prefix:name` or an unqualified name.
     *
     * A prefixed attribute is bound to the resource id the inflater resolves it
     * by: the framework table for `android:`, the app's own `attr` resources for
     * every other prefix, which the document must declare as a namespace. An
     * attribute with no id would be written and then ignored, so a name that
     * resolves to none fails the patch instead. [value] is read as resource
     * XML reads it, including the enum and flag names the attribute defines
     * (`center`, `top|start`).
     */
    operator fun set(attr: String, value: String) = xmlSetAttribute(doc, handle, attr, value)
    fun setInt(attr: String, value: Int) = set(attr, value.toString())
    fun setBool(attr: String, value: Boolean) = set(attr, value.toString())
    fun setResourceRef(attr: String, resId: UInt) = xmlSetAttributeRef(doc, handle, attr, resId)
    fun removeAttribute(name: String) = xmlRemoveAttribute(doc, handle, name)

    fun appendChild(child: XmlElement) {
        require(doc == child.doc) { "Cannot append a child of another XML document; adopt it first" }
        xmlAppendChild(doc, handle, child.handle)
    }

    /**
     * Moves [child] in front of [before] and returns [child] as it is now reachable.
     * A created or adopted element is used up by attaching it; handles resolved
     * before the call may have moved.
     */
    fun insertBefore(child: XmlElement, before: XmlElement): XmlElement {
        require(doc == child.doc && doc == before.doc) { "Cannot insert elements of another XML document; adopt them first" }
        return XmlElement(doc, xmlInsertBefore(doc, child.handle, before.handle))
    }

    fun remove() = xmlRemoveElement(doc, handle)
    fun clone(deep: Boolean = true): XmlElement = XmlElement(doc, xmlCloneElement(doc, handle, deep))
}

/** A manifest attribute such as `@0x7f1400a0` as a resource id. */
fun resourceRef(value: String): UInt? = when {
    value.startsWith("@0x") -> value.removePrefix("@0x").toUIntOrNull(16)
    value.startsWith("@ref/0x") -> value.removePrefix("@ref/0x").toUIntOrNull(16)
    else -> null
}
