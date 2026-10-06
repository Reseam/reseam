// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import java.util.zip.ZipFile
import javax.inject.Inject
import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.Project
import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.FileCollection
import org.gradle.api.tasks.Classpath
import org.gradle.api.tasks.InputFiles
import org.gradle.api.tasks.Optional
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.PathSensitive
import org.gradle.api.tasks.PathSensitivity
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations

internal abstract class DexTask @Inject constructor(private val exec: ExecOperations) :
    DefaultTask() {
    @get:Classpath val compiler: FileCollection = project.d8Classpath()

    @get:InputFiles abstract val sources: ConfigurableFileCollection

    /** Android libraries the extension ships; their native libraries cannot be carried. */
    @get:InputFiles
    @get:PathSensitive(PathSensitivity.NAME_ONLY)
    abstract val shippedAars: ConfigurableFileCollection

    /** Classes referenced but not dexed, so d8 can desugar against them. */
    @get:Classpath abstract val libraries: ConfigurableFileCollection

    @get:OutputDirectory abstract val output: DirectoryProperty

    @get:OutputDirectory @get:Optional abstract val globals: DirectoryProperty

    @TaskAction
    fun run() {
        shippedAars.files.forEach { aar ->
            ZipFile(aar).use { zip ->
                if (zip.entries().asSequence().any { it.name.startsWith("jni/") })
                    throw GradleException(
                        "${aar.name} ships native libraries, which an extension cannot carry"
                    )
            }
        }
        val outDir = output.get().asFile.also(::recreate)
        val files =
            sources.asFileTree.files.filter { it.extension == "class" } +
                sources.files.filter { it.extension == "jar" }
        check(files.isNotEmpty()) { "nothing to dex in ${project.path}" }
        val globalsDir = globals.orNull?.asFile?.also(::recreate)
        if (globalsDir == null) {
            exec.d8(compiler, libraries, outDir, files)
            return
        }
        // d8 finalizes an intermediate compile's references to its global synthetics; the
        // definitions come only from the shared globals compile.
        val intermediate = File(temporaryDir, "intermediate").also(::recreate)
        exec.d8(
            compiler,
            libraries,
            intermediate,
            files,
            "--intermediate",
            "--globals-output",
            globalsDir.absolutePath,
        )
        exec.d8(
            compiler,
            libraries,
            outDir,
            intermediate.listFiles().orEmpty().filter { it.extension == "dex" },
        )
    }
}

internal fun recreate(dir: File) {
    if (dir.exists() && !dir.deleteRecursively())
        throw org.gradle.api.GradleException("cannot remove $dir")
    if (!dir.mkdirs() && !dir.isDirectory)
        throw org.gradle.api.GradleException("cannot create $dir")
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
