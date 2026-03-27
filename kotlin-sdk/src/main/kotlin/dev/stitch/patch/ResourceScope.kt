@file:Suppress("unused")

package dev.stitch.patch

object Resources {
    val hasResources: Boolean get() = resHasResources()

    fun getString(index: Int): String? = resGetString(index.toUInt())
    fun setString(index: Int, value: String) = resSetString(index.toUInt(), value)

    fun resourceId(resType: String, resName: String): Long? =
        resResourceId(resType, resName)

    fun findEntriesByString(stringIndex: Int): List<ResourceRef> =
        resFindEntriesByString(stringIndex.toUInt())

    fun addStringResource(name: String, value: String): Int? =
        resAddStringResource(name, value)?.toInt()

    fun replaceEntryString(resId: Int, newStringIndex: Int) =
        resReplaceEntryString(resId.toUInt(), newStringIndex.toUInt())

    fun copyFile(bundlePath: String, apkPath: String) =
        resCopyFile(bundlePath, apkPath)

    fun copyResourceGroup(resType: String, files: List<String>) =
        resCopyResourceGroup(resType, files)

    fun deleteFile(apkPath: String) = resDeleteFile(apkPath)
    fun listFiles(prefix: String): List<String> = resListFiles(prefix)
}
