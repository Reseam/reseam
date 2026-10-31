// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch

import app.reseam.patch.native.bundleFilePath
import app.reseam.patch.native.checkInvocation
import app.reseam.patch.native.outputComponents
import app.reseam.patch.native.outputPath
import app.reseam.patch.native.outputReplace
import app.reseam.patch.native.outputTempFile
import java.io.File

/**
 * What a patch sees in [PatchBuilder.afterWrite]: the APK files the engine wrote, unsigned.
 * Constructed by the engine for each call and passed as the receiver.
 */
class OutputRuntime internal constructor() {
    /** The base APK; [OutputFiles.component] reaches the others. */
    val output: OutputFiles = OutputFiles(null)
    val options: RuntimeOptions = RuntimeOptions()
    val log: PatchLogger = PatchLogger()

    /** A file shipped in the bundle, at the same path [FileScope.copy] takes. */
    fun bundleFile(path: String): File = File(bundleFilePath(path))

    /**
     * A path for a new file called [name], in a directory of its own that the engine removes after
     * the run. It is on the same file system as the output, so [OutputFiles.replace] moves it.
     */
    fun tempFile(name: String): File = File(outputTempFile(name))

    @JvmName("invokeAfterWrite")
    internal fun invokeAfterWrite(patch: ReseamPatch) {
        patch.afterWrite(this)
        checkInvocation()
    }
}

/** One written APK, the base unless reached through [component]. */
class OutputFiles internal constructor(private val componentName: String?) {
    /** Names of every component, the same names [FileScope.components] lists. */
    fun components(): List<String> = outputComponents()

    fun component(name: String): OutputFiles = OutputFiles(name)

    /** The written APK, unsigned. Read it; replace it with [replace] instead of writing to it. */
    val file: File
        get() = File(outputPath(componentName))

    /** Moves [replacement] over this APK. The engine signs whatever file is in place at the end. */
    fun replace(replacement: File) = outputReplace(componentName, replacement.path)
}
