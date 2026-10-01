// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.gradle

import javax.inject.Inject
import org.gradle.api.DefaultTask
import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.FileCollection
import org.gradle.api.tasks.Classpath
import org.gradle.api.tasks.InputFiles
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations

internal abstract class GlobalSyntheticsDexTask
@Inject
constructor(private val exec: ExecOperations) : DefaultTask() {
    @get:Classpath val compiler: FileCollection = project.d8Classpath()

    /** The `classes.globals` files d8 wrote per extension. */
    @get:InputFiles abstract val globals: ConfigurableFileCollection

    @get:Classpath abstract val libraries: ConfigurableFileCollection

    @get:OutputDirectory abstract val output: DirectoryProperty

    @TaskAction
    fun run() {
        val outDir = output.get().asFile.also(::recreate)
        val files = globals.files.filter { it.isFile }
        if (files.isEmpty()) return
        exec.d8(
            compiler,
            libraries,
            outDir,
            emptyList(),
            *files.flatMap { listOf("--globals", it.absolutePath) }.toTypedArray(),
        )
    }
}
