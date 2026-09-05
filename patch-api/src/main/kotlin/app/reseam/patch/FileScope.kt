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
