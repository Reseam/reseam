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
 * The classes of [traced] that [roots] reach, as class files R8 leaves them, with the classes it
 * adds for them. R8 keeps the roots whole and neither optimizes nor renames, so the roots, dexed
 * separately, still find every member they use.
 */
internal abstract class ShrinkTask @Inject constructor(private val exec: ExecOperations) :
    DefaultTask() {
    @get:Classpath val compiler: FileCollection = project.d8Classpath()

    @get:InputFiles abstract val roots: ConfigurableFileCollection

    @get:InputFiles abstract val traced: ConfigurableFileCollection

    @get:InputFiles abstract val ruleSources: ConfigurableFileCollection

    @get:Classpath abstract val libraries: ConfigurableFileCollection

    @get:Input abstract val rules: Property<String>

    @get:OutputDirectory abstract val output: DirectoryProperty

    init {
        rules.convention(
            """
            -dontobfuscate
            -dontoptimize
            -dontwarn **
            -keepattributes *
            """
                .trimIndent()
        )
    }

    @TaskAction
    fun run() {
        val outDir = output.get().asFile.also(::recreate)
        val rootClasses = roots.classNames()
        val tracedClasses = traced.classNames() - rootClasses
        if (tracedClasses.isEmpty()) return
        val work = temporaryDir.also(::recreate)
        val program = work.resolve("program.jar")
        jar(program, (roots.files + traced.files).filter(File::isDirectory))
        val rulesFile =
            work.resolve("rules.pro").apply {
                writeText(
                    (listOf(rules.get()) +
                            ruleSources.files.flatMap(::keepRules) +
                            rootClasses.map { "-keep class ${it.replace('/', '.')} { *; }" })
                        .joinToString("\n")
                )
            }
        val shrunk = work.resolve("shrunk.jar")
        exec.javaexec {
            classpath = compiler
            mainClass.set("com.android.tools.r8.R8")
            args(
                listOf(
                    "--classfile",
                    // d8 desugars the result with the roots.
                    "--no-desugaring",
                    "--output",
                    shrunk.absolutePath,
                    "--pg-conf",
                    rulesFile.absolutePath,
                ) +
                    libraryJars(work).flatMap { listOf("--lib", it.absolutePath) } +
                    program.absolutePath
            )
        }
        ZipFile(shrunk).use { zip ->
            zip.entries()
                .asSequence()
                .map { it to it.name.removeSuffix(".class") }
                // R8 adds classes for the code it keeps, named after their class. The roots ship
                // as they were compiled, so the classes added for them are left out.
                .filter { (_, name) ->
                    name !in rootClasses &&
                        (name in tracedClasses || name.substringBeforeLast('$') !in rootClasses)
                }
                .map { (entry, _) -> entry }
                .forEach { entry ->
                    val file = outDir.resolve(entry.name)
                    file.parentFile.mkdirs()
                    zip.getInputStream(entry).use { input ->
                        file.outputStream().use(input::copyTo)
                    }
                }
        }
    }

    /** R8 reads libraries from jars only. */
    private fun libraryJars(work: File): List<File> =
        libraries.files.filter(File::exists).mapIndexed { index, library ->
            if (library.isDirectory)
                work.resolve("library-$index.jar").also { jar(it, listOf(library)) }
            else library
        }

    private fun FileCollection.classNames(): Set<String> =
        files
            .filter(File::isDirectory)
            .flatMap { dir ->
                dir.classFiles().map {
                    it.relativeTo(dir).invariantSeparatorsPath.removeSuffix(".class")
                }
            }
            .toSet()

    private fun keepRules(file: File): List<String> =
        if (!file.isFile || file.extension !in setOf("jar", "aar")) emptyList()
        else
            ZipFile(file).use { zip ->
                zip.entries()
                    .asSequence()
                    .filter {
                        !it.isDirectory &&
                            (it.name == "proguard.txt" || it.name.startsWith("META-INF/proguard/"))
                    }
                    .map { entry -> zip.getInputStream(entry).use { it.reader().readText() } }
                    .toList()
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
