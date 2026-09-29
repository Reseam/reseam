// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.gradle

import org.gradle.api.DefaultTask
import org.gradle.api.Project
import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.FileCollection
import org.gradle.api.tasks.Classpath
import org.gradle.api.tasks.InputFiles
import org.gradle.api.tasks.Optional
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations
import java.io.File
import javax.inject.Inject

/**
 * Runs d8 over class files or jars into `classes*.dex` files in the output directory.
 *
 * With [globals] set, d8's global synthetics (helpers such as the supertype of desugared
 * records) go to that directory instead of into the DEX, so modules that are linked into one
 * app separately can share a single copy, built by [GlobalSyntheticsDexTask].
 */
abstract class DexTask @Inject constructor(private val exec: ExecOperations) : DefaultTask() {
    @get:Classpath
    val compiler: FileCollection = project.d8Classpath()

    @get:InputFiles
    abstract val sources: ConfigurableFileCollection

    /** Classes referenced but not dexed, so d8 can desugar against them. */
    @get:Classpath
    abstract val libraries: ConfigurableFileCollection

    @get:OutputDirectory
    abstract val output: DirectoryProperty

    @get:OutputDirectory
    @get:Optional
    abstract val globals: DirectoryProperty

    @TaskAction
    fun run() {
        val outDir = output.get().asFile.also(::recreate)
        val files = sources.asFileTree.files.filter { it.extension == "class" } + sources.files.filter { it.extension == "jar" }
        check(files.isNotEmpty()) { "nothing to dex in ${project.path}" }
        val globalsDir = globals.orNull?.asFile?.also(::recreate)
        if (globalsDir == null) {
            exec.d8(compiler, libraries, outDir, files)
            return
        }
        // d8 finalizes an intermediate compile's references to its global synthetics; the
        // definitions come only from the shared globals compile.
        val intermediate = File(temporaryDir, "intermediate").also(::recreate)
        exec.d8(compiler, libraries, intermediate, files, "--intermediate", "--globals-output", globalsDir.absolutePath)
        exec.d8(compiler, libraries, outDir, intermediate.listFiles().orEmpty().filter { it.extension == "dex" })
    }
}

internal fun recreate(dir: File) {
    dir.deleteRecursively()
    dir.mkdirs()
}

/** Pinned with Kotlin: installed Android build-tools may contain an older d8. */
internal fun Project.d8Classpath(): FileCollection =
    objects.fileCollection().from(
        configurations.detachedConfiguration(dependencies.create("com.android.tools:r8:9.4.17")),
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
                inputs.map { it.absolutePath },
        )
    }
}
