// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream
import org.gradle.api.Project
import org.gradle.api.file.FileCollection
import org.gradle.process.ExecOperations

/**
 * Copies the classes of [jar] to [copy]. d8 reads a jar's `.dex` entries as program input too, and
 * refuses a jar with both, so a library that ships DEX files as resources would not dex.
 */
internal fun classesOf(jar: File, copy: File): File {
    ZipOutputStream(copy.outputStream()).use { out ->
        libraryClasses(jar) { classes ->
            classes.forEach { entry ->
                out.putNextEntry(ZipEntry(entry.path))
                entry.read { it.copyTo(out) }
                out.closeEntry()
            }
        }
    }
    return copy
}

internal fun Project.d8Classpath(): FileCollection =
    objects
        .fileCollection()
        .from(
            configurations.detachedConfiguration(dependencies.create("com.android.tools:r8:9.4.27"))
        )

internal fun ExecOperations.d8(
    compiler: FileCollection,
    libraries: FileCollection,
    output: File,
    inputs: List<File>,
    vararg options: String,
) {
    javaexec {
        classpath = compiler
        mainClass.set("com.android.tools.r8.D8")
        args(
            listOf("--release", "--min-api", MIN_API.toString(), "--output", output.absolutePath) +
                options +
                libraries.files.flatMap { listOf("--lib", it.absolutePath) } +
                inputs.map { it.absolutePath }
        )
    }
}
