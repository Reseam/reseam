@file:Suppress("unused")

package dev.stitch.patch

class FileScope internal constructor(
    private val componentName: String? = null,
) {
    fun components(): List<String> = fileComponentNames()

    fun component(name: String): FileScope = FileScope(name)

    fun list(): List<String> =
        componentName?.let(::fileListInComponent) ?: fileList()

    fun read(path: String): ByteArray? =
        componentName?.let { fileReadInComponent(it, path) } ?: fileRead(path)

    fun write(path: String, data: ByteArray) {
        if (componentName == null) {
            fileInject(path, data)
        } else {
            fileInjectInComponent(componentName, path, data)
        }
    }

    fun delete(path: String) {
        if (componentName == null) {
            fileDelete(path)
        } else {
            fileDeleteInComponent(componentName, path)
        }
    }

    fun copy(bundlePath: String, apkPath: String) {
        if (componentName == null) {
            fileCopy(bundlePath, apkPath)
        } else {
            fileCopyInComponent(componentName, bundlePath, apkPath)
        }
    }

    fun xml(path: String): XmlDocument {
        val handle = componentName?.let { xmlOpenInComponent(it, path) } ?: xmlOpen(path)
        return XmlDocument(handle ?: error("failed to open XML document: $path"))
    }

    fun useXml(path: String, block: XmlDocument.() -> Unit) {
        xml(path).use(block)
    }
}
