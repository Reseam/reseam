// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

class XmlDocument(val handle: UInt) : AutoCloseable {
    val root: XmlElement get() {
        requireActivePatchContext()
        return XmlElement(handle, xmlRoot(handle))
    }

    fun findByTag(tag: String): List<XmlElement> =
        run {
            requireActivePatchContext()
            xmlFindByTag(handle, tag).map { XmlElement(handle, it.toUInt()) }
        }

    fun findByAttribute(name: String, value: String): List<XmlElement> =
        run {
            requireActivePatchContext()
            xmlFindByAttribute(handle, name, value).map { XmlElement(handle, it.toUInt()) }
        }

    fun createElement(tag: String): XmlElement =
        run {
            requireActivePatchContext()
            XmlElement(handle, xmlCreateElement(handle, tag))
        }

    override fun close() = run {
        requireActivePatchContext()
        xmlClose(handle)
    }
}

class XmlElement(val doc: UInt, val handle: UInt) {
    val tag: String get() {
        requireActivePatchContext()
        return xmlTagName(doc, handle)
    }
    val parent: XmlElement? get() {
        requireActivePatchContext()
        return xmlParent(doc, handle)?.let { XmlElement(doc, it) }
    }
    val children: List<XmlElement> get() {
        requireActivePatchContext()
        return xmlChildren(doc, handle).map { XmlElement(doc, it.toUInt()) }
    }

    operator fun get(attr: String): String? = run {
        requireActivePatchContext()
        xmlGetAttribute(doc, handle, attr)
    }
    operator fun set(attr: String, value: String) = run {
        requireActivePatchContext()
        xmlSetAttribute(doc, handle, attr, value)
    }
    fun setInt(attr: String, value: Int) = run {
        requireActivePatchContext()
        xmlSetAttributeInt(doc, handle, attr, value)
    }
    fun setBool(attr: String, value: Boolean) = run {
        requireActivePatchContext()
        xmlSetAttributeBool(doc, handle, attr, value)
    }
    fun setResourceRef(attr: String, resId: UInt) = run {
        requireActivePatchContext()
        xmlSetAttributeRef(doc, handle, attr, resId)
    }

    fun removeAttribute(name: String) = run {
        requireActivePatchContext()
        xmlRemoveAttribute(doc, handle, name)
    }

    fun appendChild(child: XmlElement) = run {
        require(doc == child.doc) { "Cannot append child from a different XML document" }
        requireActivePatchContext()
        xmlAppendChild(doc, handle, child.handle)
    }
    fun insertBefore(child: XmlElement, before: XmlElement) =
        run {
            require(doc == child.doc && doc == before.doc) {
                "Cannot insert elements from different XML documents"
            }
            requireActivePatchContext()
            xmlInsertBefore(doc, handle, child.handle, before.handle)
        }

    fun remove() = run {
        requireActivePatchContext()
        xmlRemoveElement(doc, handle)
    }
    fun clone(deep: Boolean = true): XmlElement = run {
        requireActivePatchContext()
        XmlElement(doc, xmlCloneElement(doc, handle, deep))
    }
}
