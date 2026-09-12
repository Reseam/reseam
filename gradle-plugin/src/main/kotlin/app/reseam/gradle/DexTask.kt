// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.gradle

import org.gradle.api.DefaultTask
import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.tasks.Classpath
import org.gradle.api.tasks.InputFiles
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations
import javax.inject.Inject

/** Runs d8 over class files or jars into `classes*.dex` files in the output directory. */
abstract class DexTask @Inject constructor(private val exec: ExecOperations) : DefaultTask() {
    @get:InputFiles
    abstract val sources: ConfigurableFileCollection

    /** Classes referenced but not dexed, so d8 can desugar against them. */
    @get:Classpath
    abstract val libraries: ConfigurableFileCollection

    @get:OutputDirectory
    abstract val output: DirectoryProperty

    @TaskAction
    fun run() {
        val outDir = output.get().asFile
        outDir.deleteRecursively()
        outDir.mkdirs()
        val files = sources.asFileTree.files.filter { it.extension == "class" } + sources.files.filter { it.extension == "jar" }
        check(files.isNotEmpty()) { "nothing to dex in ${project.path}" }
        exec.exec {
            commandLine(
                listOf(AndroidSdk.d8().absolutePath, "--release", "--min-api", MIN_API.toString(), "--output", outDir.absolutePath) +
                    libraries.files.flatMap { listOf("--lib", it.absolutePath) } +
                    files.map { it.absolutePath },
            )
        }
    }
}
