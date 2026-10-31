// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.tasks.InputFiles
import org.gradle.api.tasks.PathSensitive
import org.gradle.api.tasks.PathSensitivity
import org.gradle.api.tasks.TaskAction
import org.objectweb.asm.ClassReader

/**
 * Fails when a public signature of a standalone extension, such as the app runtime, names a Kotlin
 * stdlib type: the module and each bundle that calls it relocate the stdlib differently.
 */
internal abstract class KotlinFreeApiTask : DefaultTask() {
    @get:InputFiles
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val classes: ConfigurableFileCollection

    @TaskAction
    fun run() {
        val scanned =
            classes.classFiles().map { file ->
                ApiScanner().apply { ClassReader(file.readBytes()).accept(this, SKIP) }
            }
        val hiddenClasses = scanned.filterNot { it.visible }.map { it.className }.toSet()
        // A nested class is only as visible as the classes around it.
        val problems =
            scanned
                .filter { scanner -> hiddenClasses.none { scanner.className.startsWith("$it$") } }
                .flatMap { it.problems }
        if (problems.isNotEmpty())
            throw GradleException(
                "Public signatures name Kotlin stdlib types:\n" + problems.joinToString("\n")
            )
    }

    private companion object {
        const val SKIP = ClassReader.SKIP_CODE or ClassReader.SKIP_DEBUG or ClassReader.SKIP_FRAMES
    }
}
