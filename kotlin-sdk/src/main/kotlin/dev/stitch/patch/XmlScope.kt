@file:Suppress("unused")

package dev.stitch.patch

class XmlDocument(val handle: UInt) : AutoCloseable {
    val root: XmlElement get() = XmlElement(xmlRoot(handle))

    fun findByTag(tag: String): List<XmlElement> =
        xmlFindByTag(handle, tag).map { XmlElement(it.toUInt()) }

    fun findByAttribute(name: String, value: String): List<XmlElement> =
        xmlFindByAttribute(handle, name, value).map { XmlElement(it.toUInt()) }

    fun createElement(tag: String): XmlElement =
        XmlElement(xmlCreateElement(handle, tag))

    override fun close() = xmlClose(handle)
}

class XmlElement(val handle: UInt) {
    val tag: String get() = xmlTagName(handle)
    val parent: XmlElement? get() = xmlParent(handle)?.let { XmlElement(it) }
    val children: List<XmlElement> get() = xmlChildren(handle).map { XmlElement(it.toUInt()) }

    operator fun get(attr: String): String? = xmlGetAttribute(handle, attr)
    operator fun set(attr: String, value: String) = xmlSetAttribute(handle, attr, value)

    fun removeAttribute(name: String) = xmlRemoveAttribute(handle, name)

    fun appendChild(child: XmlElement) = xmlAppendChild(handle, child.handle)
    fun insertBefore(child: XmlElement, before: XmlElement) =
        xmlInsertBefore(handle, child.handle, before.handle)

    fun remove() = xmlRemoveElement(handle)
    fun clone(deep: Boolean = true): XmlElement = XmlElement(xmlCloneElement(handle, deep))
}

fun xmlDocument(apkPath: String, block: XmlDocument.() -> Unit) {
    val docHandle = xmlOpen(apkPath)
        ?: error("failed to open XML document: $apkPath")
    XmlDocument(docHandle).use(block)
}
