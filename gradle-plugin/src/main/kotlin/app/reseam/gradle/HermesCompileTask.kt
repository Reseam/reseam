// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import javax.inject.Inject
import org.gradle.api.DefaultTask
import org.gradle.api.file.ArchiveOperations
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.FileCollection
import org.gradle.api.file.FileSystemOperations
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.InputFile
import org.gradle.api.tasks.InputFiles
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.PathSensitive
import org.gradle.api.tasks.PathSensitivity
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations

/** The pinned Hermes compiler release and the bytecode version it emits. */
internal object HermesCompiler {
    const val BYTECODE_VERSION = 98
    const val PACKAGE = "hermes-compiler"
    const val TASK = "hermes$BYTECODE_VERSION"

    /** Where a bundle carries the bytecode, as the engine loads it. */
    const val BUNDLE_FOLDER = "resources/hermes/v$BYTECODE_VERSION"

    fun tool() =
        NpmTool(
            NpmPackage(PACKAGE, "250829098.0.19"),
            "sha512-qMdHsuntdFVNqx3EPOZyyQd4qHD2g0o3y32SYX2Z7jKG1uF7B2WDBHeZK/6qOnl06BQiXPhBuD6SdZv10ngtyw==",
            "hermesc/" +
                when (Host.current) {
                    Host.LINUX_X64 -> "linux64-bin/hermesc"
                    Host.MACOS_X64,
                    Host.MACOS_ARM64 -> "osx-bin/hermesc"
                    Host.WINDOWS_X64 -> "win64-bin/hermesc.exe"
                },
        )
}

internal abstract class HermesCompileTask
@Inject
constructor(
    private val exec: ExecOperations,
    private val archives: ArchiveOperations,
    private val files: FileSystemOperations,
) : DefaultTask() {
    @get:InputFiles
    @get:PathSensitive(PathSensitivity.NONE)
    val compiler: FileCollection = project.npmArchive { HermesCompiler.tool().pkg }

    @get:InputFile @get:PathSensitive(PathSensitivity.NONE) abstract val script: RegularFileProperty
    @get:Input abstract val artifact: Property<String>
    @get:OutputDirectory abstract val output: DirectoryProperty

    @TaskAction
    fun compile() {
        val executable =
            HermesCompiler.tool()
                .unpack(compiler.singleFile, File(temporaryDir, "compiler"), archives, files)
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
                script.get().asFile.absolutePath,
            )
        }
    }
}
