@file:Suppress("unused")

package dev.stitch.patch

class XmlDocument(val handle: UInt) : AutoCloseable {
    val root: XmlElement get() = XmlElement(handle, xmlRoot(handle))

    fun findByTag(tag: String): List<XmlElement> =
        xmlFindByTag(handle, tag).map { XmlElement(handle, it.toUInt()) }

    fun findByAttribute(name: String, value: String): List<XmlElement> =
        xmlFindByAttribute(handle, name, value).map { XmlElement(handle, it.toUInt()) }

    fun createElement(tag: String): XmlElement =
        XmlElement(handle, xmlCreateElement(handle, tag))

    override fun close() = xmlClose(handle)
}

class XmlElement(val doc: UInt, val handle: UInt) {
    val tag: String get() = xmlTagName(doc, handle)
    val parent: XmlElement? get() = xmlParent(doc, handle)?.let { XmlElement(doc, it) }
    val children: List<XmlElement> get() = xmlChildren(doc, handle).map { XmlElement(doc, it.toUInt()) }

    operator fun get(attr: String): String? = xmlGetAttribute(doc, handle, attr)
    operator fun set(attr: String, value: String) = xmlSetAttribute(doc, handle, attr, value)

    fun removeAttribute(name: String) = xmlRemoveAttribute(doc, handle, name)

    fun appendChild(child: XmlElement) = xmlAppendChild(doc, handle, child.handle)
    fun insertBefore(child: XmlElement, before: XmlElement) =
        xmlInsertBefore(doc, handle, child.handle, before.handle)

    fun remove() = xmlRemoveElement(doc, handle)
    fun clone(deep: Boolean = true): XmlElement = XmlElement(doc, xmlCloneElement(doc, handle, deep))
}

fun xmlDocument(apkPath: String, block: XmlDocument.() -> Unit) {
    val docHandle = xmlOpen(apkPath)
        ?: error("failed to open XML document: $apkPath")
    XmlDocument(docHandle).use(block)
}
