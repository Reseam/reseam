// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import java.util.zip.ZipEntry
import java.util.zip.ZipFile
import java.util.zip.ZipOutputStream
import javax.inject.Inject
import org.gradle.api.DefaultTask
import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.FileCollection
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Classpath
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.InputFiles
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations

internal abstract class RelocateKotlinTask : DefaultTask() {
    @get:InputFiles abstract val classes: ConfigurableFileCollection

    /** Stubs of the app's classes, which keep their names. */
    @get:InputFiles abstract val appClasses: ConfigurableFileCollection

    @get:Input abstract val kotlinPackage: Property<String>

    @get:OutputDirectory abstract val output: DirectoryProperty

    @TaskAction
    fun run() {
        val outDir = output.get().asFile.also(::recreate)
        val app =
            appClasses.files
                .filter(File::isDirectory)
                .flatMap { dir ->
                    dir.classFiles()
                        .map { it.relativeTo(dir).invariantSeparatorsPath.removeSuffix(".class") }
                        .toList()
                }
                .toSet()
        KotlinRelocation(kotlinPackage.get(), app).relocate(classes.files, outDir)
    }
}

/**
 * The part of the Kotlin standard library that [roots] use, as class files. R8 traces from the
 * roots but neither optimizes nor renames, so the roots, dexed separately, still find every member
 * they call.
 */
internal abstract class KotlinStdlibTask @Inject constructor(private val exec: ExecOperations) :
    DefaultTask() {
    @get:Classpath val compiler: FileCollection = project.d8Classpath()

    @get:InputFiles abstract val roots: ConfigurableFileCollection

    /** The standard library, relocated like the roots. */
    @get:InputFiles abstract val stdlib: ConfigurableFileCollection

    @get:Classpath abstract val libraries: ConfigurableFileCollection

    @get:Input abstract val kotlinPackage: Property<String>

    /** R8's configuration: keep the roots whole and trace the standard library from them. */
    @get:Input abstract val rules: Property<String>

    @get:OutputDirectory abstract val output: DirectoryProperty

    init {
        rules.convention(
            kotlinPackage.map { prefix ->
                """
                -dontobfuscate
                -dontoptimize
                -dontwarn **
                -keep class !$prefix.**,** { *; }
                """
                    .trimIndent()
            }
        )
    }

    @TaskAction
    fun run() {
        val outDir = output.get().asFile.also(::recreate)
        val work = temporaryDir.also(::recreate)
        val program = work.resolve("program.jar")
        jar(program, (roots.files + stdlib.files).filter(File::isDirectory))
        val prefix = kotlinPackage.get()
        val rulesFile = work.resolve("rules.pro").apply { writeText(rules.get()) }
        val shrunk = work.resolve("shrunk.jar")
        exec.javaexec {
            classpath = compiler
            mainClass.set("com.android.tools.r8.R8")
            args(
                listOf(
                    "--classfile",
                    "--output",
                    shrunk.absolutePath,
                    "--pg-conf",
                    rulesFile.absolutePath,
                ) +
                    libraries.files.flatMap { listOf("--lib", it.absolutePath) } +
                    program.absolutePath
            )
        }
        val path = prefix.replace('.', '/') + "/"
        ZipFile(shrunk).use { zip ->
            zip.entries()
                .asSequence()
                .filter { it.name.startsWith(path) && it.name.endsWith(".class") }
                .forEach { entry ->
                    val file = outDir.resolve(entry.name)
                    file.parentFile.mkdirs()
                    zip.getInputStream(entry).use { input ->
                        file.outputStream().use(input::copyTo)
                    }
                }
        }
    }

    private fun jar(target: File, directories: List<File>) {
        val written = mutableSetOf<String>()
        ZipOutputStream(target.outputStream()).use { zip ->
            for (directory in directories) {
                directory.classFiles().forEach { file ->
                    val name = file.relativeTo(directory).invariantSeparatorsPath
                    if (written.add(name)) {
                        zip.putNextEntry(ZipEntry(name))
                        file.inputStream().use { it.copyTo(zip) }
                        zip.closeEntry()
                    }
                }
            }
        }
    }
}
