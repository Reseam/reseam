// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import javax.inject.Inject
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonPrimitive
import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.file.ArchiveOperations
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.FileCollection
import org.gradle.api.file.FileSystemOperations
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.InputDirectory
import org.gradle.api.tasks.InputFiles
import org.gradle.api.tasks.OutputFile
import org.gradle.api.tasks.PathSensitive
import org.gradle.api.tasks.PathSensitivity
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations

/** The pinned esbuild release, published as one npm package per host. */
internal object Esbuild {
    const val SCOPE = "esbuild"

    fun tool() =
        when (Host.current) {
            Host.LINUX_X64 ->
                build(
                    "linux-x64",
                    "sha512-4xTZr1FUmSoQW4XIWmit3tzQrUTZM+N3P0XV8xROKYF50XfI7xeO90+1bZvNwxIufQ9hDQVRJH5YhgPVF8A/HQ==",
                    "bin/esbuild",
                )
            Host.MACOS_X64 ->
                build(
                    "darwin-x64",
                    "sha512-uq6suIWYP37qzGddBKPw5QEQPi6HiLGsO7UmkpfyaYNQ3D+rN6w6WfwH+nuqcGXWvawGwxOEroO4YGnFh95azw==",
                    "bin/esbuild",
                )
            Host.MACOS_ARM64 ->
                build(
                    "darwin-arm64",
                    "sha512-n4KqkOQrraxHJcgjM1RvwbigfQKIKJVpM7xp+KsxiyUSrRdIXnt73VhrPAx0fV44hgfmIVKjxMN9J1t5jySVkw==",
                    "bin/esbuild",
                )
            Host.WINDOWS_X64 ->
                build(
                    "win32-x64",
                    "sha512-5ebpxr3nWMzrL/rnUI755Jkuee0bHL/Gq0WTF9lvcpv73wAp5eu8MfBUgWK9bhWvZjj7yX8etf/8tI8Ney695g==",
                    "esbuild.exe",
                )
        }

    private fun build(platform: String, integrity: String, executable: String) =
        NpmTool(NpmPackage("@$SCOPE/$platform", "0.28.2"), integrity, executable)
}

@Serializable private data class Metafile(val outputs: Map<String, MetafileOutput>)

@Serializable
private data class MetafileOutput(val exports: List<String>, val imports: List<MetafileImport>)

@Serializable private data class MetafileImport(val path: String, val external: Boolean = false)

/** What a JavaScript extension exports, for the binding its patch module generates. */
@Serializable
internal data class JavascriptExports(
    /** The extension's artifact name, as `ExtJsModule` takes it. */
    val artifact: String,
    /** The binding object's name. */
    val binding: String,
    val exports: List<String>,
)

/**
 * Bundles the extension's entry module and the modules it imports into one script, the shape the
 * engine links: `{ imports, link }`. Bare specifiers stay app modules: `imports` lists them, and
 * `link(modules)` receives `{ id, esModule, keys }` for each and returns the entry's exports.
 */
internal abstract class JavascriptBundleTask
@Inject
constructor(
    private val exec: ExecOperations,
    private val archives: ArchiveOperations,
    private val files: FileSystemOperations,
) : DefaultTask() {
    @get:InputFiles
    @get:PathSensitive(PathSensitivity.NONE)
    val bundler: FileCollection = project.npmArchive { Esbuild.tool().pkg }

    @get:InputDirectory
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val sources: DirectoryProperty
    @get:Input abstract val artifact: Property<String>
    @get:Input abstract val binding: Property<String>
    @get:OutputFile abstract val script: RegularFileProperty
    @get:OutputFile abstract val exports: RegularFileProperty

    @TaskAction
    fun bundle() {
        val esbuild =
            Esbuild.tool()
                .unpack(bundler.singleFile, File(temporaryDir, "esbuild"), archives, files)
        val root = sources.get().asFile
        val entry =
            ENTRY_EXTENSIONS.map { File(root, "index.$it") }.filter(File::isFile).singleOrNull()
                ?: throw GradleException(
                    "$root needs exactly one entry module: index.${ENTRY_EXTENSIONS.joinToString("|")}"
                )
        // Only ES module output lists the entry's exports in the metafile.
        val metafile = File(temporaryDir, "metafile.json")
        bundle(
            esbuild,
            entry,
            File(temporaryDir, "module.js"),
            "--format=esm",
            "--metafile=$metafile",
        )
        val output =
            bundleJson.decodeFromString<Metafile>(metafile.readText()).outputs.values.single()
        val imports = output.imports.filter { it.external }.map { it.path }.distinct()
        val iife = File(temporaryDir, "iife.js")
        bundle(esbuild, entry, iife, "--format=iife", "--global-name=$GLOBAL")
        script.get().asFile.writeText(script(imports, output.exports, iife.readText()))
        exports
            .get()
            .asFile
            .writeText(
                bundleJson.encodeToString(
                    JavascriptExports(artifact.get(), binding.get(), output.exports)
                )
            )
    }

    /** Fills [TEMPLATE], whose bundle goes in last so no marker is read inside esbuild's output. */
    private fun script(imports: List<String>, exports: List<String>, iife: String) =
        TEMPLATE.replace("/*imports*/", imports.joinToString(", ", "[", "]", transform = ::literal))
            .replace("/*global*/", GLOBAL)
            .replace(
                "/*exports*/",
                exports.joinToString("\n" + " ".repeat(16)) {
                    "${literal(it)}: function () { return load()[${literal(it)}].apply(this, arguments); },"
                },
            )
            .replace("/*bundle*/", iife)

    private fun literal(text: String) = JsonPrimitive(text).toString()

    private fun bundle(esbuild: File, entry: File, out: File, vararg format: String) {
        exec.exec {
            commandLine(
                listOf(
                    esbuild.absolutePath,
                    entry.absolutePath,
                    "--bundle",
                    "--platform=neutral",
                    "--packages=external",
                    // Bundling follows these options alone, not the folder's tsconfig.json.
                    "--tsconfig-raw={}",
                    // hermesc compiles ES2023 except async generators.
                    "--target=es2023",
                    "--supported:async-generator=false",
                    "--jsx=automatic",
                    "--charset=utf8",
                    "--log-level=warning",
                    "--outfile=${out.absolutePath}",
                ) + format
            )
        }
    }

    private companion object {
        val ENTRY_EXTENSIONS = listOf("ts", "tsx", "js", "jsx")
        const val GLOBAL = "reseamExtension"
        /** The script the engine links, around esbuild's bundle. */
        val TEMPLATE: String by lazy {
            JavascriptBundleTask::class
                .java
                .getResource("extension.js")
                .let { it ?: throw GradleException("Reseam plugin extension template is missing") }
                .readText()
        }
    }
}
