// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.native.componentNames
import app.reseam.patch.native.fileCopy
import app.reseam.patch.native.fileDelete
import app.reseam.patch.native.fileInject
import app.reseam.patch.native.fileList
import app.reseam.patch.native.fileRead
import app.reseam.patch.native.fileSigners
import app.reseam.patch.native.fileXmlWith
import app.reseam.patch.native.xmlOpen
import app.reseam.patch.run.ApkSourceStream
import app.reseam.patch.run.EntryStream
import app.reseam.patch.run.writeStream
import app.reseam.patch.types.FileCompression
import java.io.InputStream

class FileScope internal constructor(private val componentName: String? = null) {
    fun components(): List<String> = componentNames()

    fun component(name: String): FileScope = FileScope(name)

    fun list(): List<String> = fileList(componentName)

    fun read(path: String): ByteArray? = fileRead(componentName, path)

    /**
     * A file's contents as a stream, for files too large to [read] whole. It reads the file as it
     * was when opened, changes made so far included; later changes don't show. Returns null when
     * the file doesn't exist. The stream is confined to the current patch callback; close it before
     * the callback returns.
     */
    fun stream(path: String): InputStream? = EntryStream.open(componentName, path)

    /**
     * Compiled XML files where an element carries one of [attributes], named as [XmlElement.get]
     * takes them.
     */
    fun xmlFilesWith(vararg attributes: String): List<String> =
        fileXmlWith(componentName, attributes.toList())

    /**
     * Streams original APK bytes, including the signing block, in bounded reads. The stream is
     * confined to the current patch callback and must be closed before that callback returns.
     * Staged edits do not affect it. Native read failures are propagated to the caller.
     */
    fun sourceStream(): InputStream = ApkSourceStream(componentName)

    /**
     * The original signer certificates, DER-encoded X.509: v3 signers when present, otherwise v2;
     * empty when unsigned or v1-only.
     */
    fun signers(): List<ByteArray> = fileSigners(componentName)

    fun write(path: String, data: ByteArray) =
        fileInject(componentName, path, data, FileCompression.DEFLATED)

    fun writeStored(path: String, data: ByteArray) =
        fileInject(componentName, path, data, FileCompression.STORED)

    /**
     * Adds or replaces a file with everything [data] reads, compressed, without holding it all in
     * memory. The file stays as it was when reading [data] fails. [data] stays open.
     */
    fun write(path: String, data: InputStream) =
        writeStream(componentName, path, data, FileCompression.DEFLATED)

    /** [write] for a file Android needs uncompressed, from a stream. */
    fun writeStored(path: String, data: InputStream) =
        writeStream(componentName, path, data, FileCompression.STORED)

    fun delete(path: String) = fileDelete(componentName, path)

    /** Copies a file shipped in the bundle into the APK. */
    fun copy(bundlePath: String, apkPath: String) = fileCopy(componentName, bundlePath, apkPath)

    fun xml(path: String): XmlDocument =
        XmlDocument(xmlOpen(componentName, path) ?: error("failed to open XML document: $path"))

    fun <T> editXml(path: String, block: XmlDocument.() -> T): T = xml(path).use(block)
}
