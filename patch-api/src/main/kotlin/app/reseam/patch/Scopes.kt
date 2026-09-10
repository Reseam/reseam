// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.dex.DexClass
import app.reseam.patch.dex.Method

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

    /** Rewrites every `const-string` equal to `old` in the app; returns how many methods changed. */
    fun replaceAllStrings(old: String, new: String): Int =
        findInstructionsByString(old)
            .map { Method(it.method) }
            .distinctBy { it.handle }
            .sumOf { it.replaceAllStrings(old, new) }
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

class ResourceScope internal constructor(private val componentName: String? = null) {
    fun components(): List<String> = resComponentNames()
    fun component(name: String): ResourceScope = ResourceScope(name)

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

    fun poolGet(index: UInt): String? = resPoolGet(componentName, index)
    fun poolSet(index: UInt, value: String) = resPoolSet(componentName, index, value)
    fun poolAdd(value: String): UInt? = resPoolAdd(componentName, value)
    fun poolFindRefs(stringIndex: UInt): List<ResourceRef> = resPoolFindRefs(componentName, stringIndex)
    fun replaceEntry(resId: UInt, newStringIndex: UInt) = resReplaceEntry(componentName, resId, newStringIndex)
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
    fun setAttributeString(elementName: String, attrName: String, value: String) = manifestSetAttributeString(componentName, elementName, attrName, value)
    fun setActivityConfigChanges(activityName: String, configChanges: String) = manifestSetActivityConfigChanges(componentName, activityName, configChanges)
    fun addIntentFilter(activityName: String, action: String? = null, category: String? = null, mimeType: String? = null) =
        manifestAddIntentFilter(componentName, activityName, action, category, mimeType)
    fun addActivityAlias(targetActivity: String, aliasName: String, enabled: Boolean = true, label: String? = null) =
        manifestAddActivityAlias(componentName, targetActivity, aliasName, enabled, label)
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
    fun findByTag(tag: String): List<XmlElement> = xmlFindByTag(handle, tag).map { XmlElement(handle, it.toUInt()) }
    fun findByAttribute(name: String, value: String): List<XmlElement> = xmlFindByAttribute(handle, name, value).map { XmlElement(handle, it.toUInt()) }
    fun createElement(tag: String): XmlElement = XmlElement(handle, xmlCreateElement(handle, tag))
    override fun close() = xmlClose(handle)
}

class XmlElement(val doc: UInt, val handle: UInt) {
    val tag: String get() = xmlTagName(doc, handle)
    val parent: XmlElement? get() = xmlParent(doc, handle)?.let { XmlElement(doc, it) }
    val children: List<XmlElement> get() = xmlChildren(doc, handle).map { XmlElement(doc, it.toUInt()) }

    operator fun get(attr: String): String? = xmlGetAttribute(doc, handle, attr)
    operator fun set(attr: String, value: String) = xmlSetAttribute(doc, handle, attr, value)
    fun setInt(attr: String, value: Int) = set(attr, value.toString())
    fun setBool(attr: String, value: Boolean) = set(attr, value.toString())
    fun setResourceRef(attr: String, resId: UInt) = xmlSetAttributeRef(doc, handle, attr, resId)
    fun removeAttribute(name: String) = xmlRemoveAttribute(doc, handle, name)

    fun appendChild(child: XmlElement) {
        require(doc == child.doc) { "Cannot append child from a different XML document" }
        xmlAppendChild(doc, handle, child.handle)
    }

    fun insertBefore(child: XmlElement, before: XmlElement) {
        require(doc == child.doc && doc == before.doc) { "Cannot insert elements from different XML documents" }
        xmlInsertBefore(doc, child.handle, before.handle)
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
