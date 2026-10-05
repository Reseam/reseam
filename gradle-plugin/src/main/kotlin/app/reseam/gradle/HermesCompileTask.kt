// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import java.io.OutputStream
import java.security.DigestInputStream
import java.security.MessageDigest
import java.util.HexFormat
import javax.inject.Inject
import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.file.ArchiveOperations
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.FileCollection
import org.gradle.api.file.FileSystemOperations
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Classpath
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.InputDirectory
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.PathSensitive
import org.gradle.api.tasks.PathSensitivity
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations

/** The pinned Hermes compiler release and the bytecode version it emits. */
internal object HermesCompiler {
    const val BYTECODE_VERSION = 98
    const val PACKAGE = "hermes-compiler:hermes-compiler:250829098.0.19@tgz"
    const val SHA256 = "839af099f0926b5ebbd738ba54764a3f40e35dfc3aac91babf56f8eb7c69280e"
    const val TASK = "hermes$BYTECODE_VERSION"
}

internal abstract class HermesCompileTask
@Inject
constructor(
    private val exec: ExecOperations,
    private val archives: ArchiveOperations,
    private val files: FileSystemOperations,
) : DefaultTask() {
    @get:Classpath
    val compiler: FileCollection =
        project.objects
            .fileCollection()
            .from(
                project.configurations.detachedConfiguration(
                    project.dependencies.create(HermesCompiler.PACKAGE)
                )
            )

    @get:InputDirectory
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val sources: DirectoryProperty
    @get:Input abstract val artifact: Property<String>
    @get:OutputDirectory abstract val output: DirectoryProperty

    @TaskAction
    fun compile() {
        val archive = compiler.singleFile
        val digest = MessageDigest.getInstance("SHA-256")
        DigestInputStream(archive.inputStream(), digest).use {
            it.copyTo(OutputStream.nullOutputStream())
        }
        if (HexFormat.of().formatHex(digest.digest()) != HermesCompiler.SHA256) {
            throw GradleException("Hermes compiler archive checksum does not match")
        }
        val tools = File(temporaryDir, "compiler")
        files.sync {
            from(archives.tarTree(archive))
            into(tools)
        }
        val platform =
            when {
                System.getProperty("os.name").startsWith("Windows") -> "win64-bin/hermesc.exe"
                System.getProperty("os.name").startsWith("Mac") -> "osx-bin/hermesc"
                System.getProperty("os.name").startsWith("Linux") -> "linux64-bin/hermesc"
                else -> throw GradleException("Hermes compiler does not support this host OS")
            }
        val executable = File(tools, "package/hermesc/$platform")
        if (!executable.setExecutable(true) && !executable.canExecute())
            throw GradleException("cannot execute $executable")
        val root = sources.get().asFile
        val javascript =
            root
                .walkTopDown()
                .filter { it.isFile && it.extension == "js" }
                .sortedBy { it.relativeTo(root).invariantSeparatorsPath }
                .toList()
        if (javascript.isEmpty())
            throw GradleException("JavaScript extension ${artifact.get()} has no .js sources")
        val source = File(temporaryDir, "${artifact.get()}.js")
        source.bufferedWriter().use { writer ->
            writer.write("(function(){\n'use strict';\nvar exports = {};\n")
            javascript.forEach { file ->
                file.bufferedReader().use { it.copyTo(writer) }
                writer.write("\n;\n")
            }
            writer.write("return exports;\n})();\n")
        }
        val out = output.get().asFile.also(::recreate)
        exec.exec {
            commandLine(
                executable.absolutePath,
                "-O",
                // Without it hermesc gives let and const function scope, so closures created in a
                // loop share one binding.
                "-Xes6-block-scoping",
                "-emit-binary",
                "-out",
                File(out, "${artifact.get()}.hbc").absolutePath,
                source.absolutePath,
            )
        }
    }
}
