// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

import app.reseam.patch.native.resId
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
import app.reseam.patch.native.xmlParent
import app.reseam.patch.native.xmlRemoveAttribute
import app.reseam.patch.native.xmlRemoveElement
import app.reseam.patch.native.xmlRoot
import app.reseam.patch.native.xmlSetAttribute
import app.reseam.patch.native.xmlSetAttributeRef
import app.reseam.patch.native.xmlTagName

class XmlDocument(val handle: UInt) : AutoCloseable {
    private var closed = false
    val root: XmlElement
        get() = XmlElement(handle, xmlRoot(handle))

    fun findByTag(tag: String): List<XmlElement> =
        xmlFindByTag(handle, tag).map { XmlElement(handle, it) }

    fun findByAttribute(name: String, value: String): List<XmlElement> =
        xmlFindByAttribute(handle, name, value).map { XmlElement(handle, it) }

    fun createElement(tag: String): XmlElement = XmlElement(handle, xmlCreateElement(handle, tag))

    /**
     * Declares [prefix] for [uri] unless the document already declares it. Existing element handles
     * continue to identify the same elements.
     */
    fun declareNamespace(prefix: String, uri: String) = xmlDeclareNamespace(handle, prefix, uri)

    /**
     * A detached deep copy of [element], which belongs to another document, in this document's
     * strings and namespaces. Attach it with [XmlElement.appendChild] or [XmlElement.insertBefore].
     * Every attribute is rebound to the resource id this document resolves it by, and one that
     * resolves to none fails the patch; declare a namespace the source uses and this document lacks
     * with [declareNamespace] first.
     */
    fun adopt(element: XmlElement): XmlElement =
        XmlElement(handle, xmlAdopt(handle, element.doc, element.handle))

    /** Releases this borrow once; other borrowers remain usable. */
    override fun close() {
        if (closed) return
        closed = true
        xmlClose(handle)
    }

    companion object {
        /**
         * XML text compiled into a document of its own, resolving `@type/name` references and
         * attribute ids against the app's resource table. It is backed by no APK entry, so closing
         * it discards it.
         */
        fun compile(text: String): XmlDocument = XmlDocument(xmlCompile(text))
    }
}

class XmlElement(val doc: UInt, val handle: UInt) {
    val tag: String
        get() = xmlTagName(doc, handle)

    val parent: XmlElement?
        get() = xmlParent(doc, handle)?.let { XmlElement(doc, it) }

    val children: List<XmlElement>
        get() = xmlChildren(doc, handle).map { XmlElement(doc, it) }

    operator fun get(attr: String): String? = xmlGetAttribute(doc, handle, attr)

    /**
     * Sets `attr`, which is `prefix:name` or an unqualified name.
     *
     * A prefixed attribute is bound to the resource id the inflater resolves it by: the framework
     * table for `android:`, the app's own `attr` resources for every other prefix, which the
     * document must declare as a namespace. An attribute with no id would be written and then
     * ignored, so a name that resolves to none fails the patch instead. [value] is read as resource
     * XML reads it, including the enum and flag names the attribute defines (`center`,
     * `top|start`).
     */
    operator fun set(attr: String, value: String) = xmlSetAttribute(doc, handle, attr, value)

    fun setInt(attr: String, value: Int) = set(attr, value.toString())

    fun setBool(attr: String, value: Boolean) = set(attr, value.toString())

    fun setResourceRef(attr: String, resId: UInt) = xmlSetAttributeRef(doc, handle, attr, resId)

    fun removeAttribute(name: String) = xmlRemoveAttribute(doc, handle, name)

    fun appendChild(child: XmlElement) {
        require(doc == child.doc) {
            "Cannot append a child of another XML document; adopt it first"
        }
        xmlAppendChild(doc, handle, child.handle)
    }

    /**
     * Moves [child] in front of [before], preserving the identities of both elements and their
     * descendants. The returned element has the same handle as [child].
     */
    fun insertBefore(child: XmlElement, before: XmlElement): XmlElement {
        require(doc == child.doc && doc == before.doc) {
            "Cannot insert elements of another XML document; adopt them first"
        }
        return XmlElement(doc, xmlInsertBefore(doc, child.handle, before.handle))
    }

    fun remove() = xmlRemoveElement(doc, handle)

    fun clone(deep: Boolean = true): XmlElement =
        XmlElement(doc, xmlCloneElement(doc, handle, deep))
}
