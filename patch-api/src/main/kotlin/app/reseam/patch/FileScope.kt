// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

class FileScope internal constructor(
    private val componentName: String? = null,
) {
    fun components(): List<String> = componentNames()

    fun component(name: String): FileScope = FileScope(name)

    fun list(): List<String> = fileList(componentName)

    fun read(path: String): ByteArray? = fileRead(componentName, path)

    /** The original, unmodified bytes of this component's APK, signing block included. */
    fun source(): ByteArray? = fileSource(componentName)

    /**
     * The original signer certificates, DER-encoded X.509, from the APK
     * Signing Block: the v3 signers when present, otherwise v2. Empty when the
     * APK is unsigned or carries only a v1 (JAR) signature.
     */
    fun signers(): List<ByteArray> = fileSigners(componentName)

    fun write(path: String, data: ByteArray) = fileInject(componentName, path, data, false)

    fun writeStored(path: String, data: ByteArray) = fileInject(componentName, path, data, true)

    fun delete(path: String) = fileDelete(componentName, path)

    fun copy(bundlePath: String, apkPath: String) = fileCopy(componentName, bundlePath, apkPath)

    fun xml(path: String): XmlDocument {
        val handle = xmlOpen(componentName, path) ?: error("failed to open XML document: $path")
        return XmlDocument(handle)
    }

    fun useXml(path: String, block: XmlDocument.() -> Unit) {
        xml(path).use(block)
    }
}
