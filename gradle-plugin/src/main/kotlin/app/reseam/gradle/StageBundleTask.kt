// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import java.io.OutputStream
import java.security.DigestInputStream
import java.security.MessageDigest
import javax.inject.Inject
import org.gradle.api.DefaultTask
import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.DuplicatesStrategy
import org.gradle.api.file.FileSystemOperations
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.provider.ListProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.InputFile
import org.gradle.api.tasks.InputFiles
import org.gradle.api.tasks.Nested
import org.gradle.api.tasks.Optional
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.PathSensitive
import org.gradle.api.tasks.PathSensitivity
import org.gradle.api.tasks.TaskAction

/** Files a module stages: [from], narrowed to [include], under [into]. */
internal abstract class StagedFiles {
    @get:InputFiles
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val from: ConfigurableFileCollection

    @get:Input @get:Optional abstract val include: Property<String>

    @get:Input abstract val into: Property<String>

    /** What d8's `classes*.dex` are renamed to, keeping their numbers. */
    @get:Input @get:Optional abstract val dexName: Property<String>

    init {
        into.convention("")
    }
}

/**
 * What files built from every module that uses them are made from besides those modules, such as
 * the stdlib a shared shrunk copy comes from. Each user's part is in that user's own files.
 */
internal abstract class ModuleSource {
    @get:InputFiles
    @get:PathSensitive(PathSensitivity.NONE)
    abstract val files: ConfigurableFileCollection

    @get:Input abstract val values: ListProperty<String>

    /** The SHA-256 of [files]' contents, with paths inside folders, and of [values]. */
    fun digest(): String {
        val entries =
            files.files
                .flatMap { root ->
                    root.walk().filter(File::isFile).map { file ->
                        val digest = MessageDigest.getInstance("SHA-256")
                        digest.update(file.relativeTo(root).invariantSeparatorsPath.toByteArray())
                        DigestInputStream(file.inputStream(), digest).use {
                            it.transferTo(OutputStream.nullOutputStream())
                        }
                        digest.digest().toHex()
                    }
                }
                .sorted()
        val digest = MessageDigest.getInstance("SHA-256")
        (entries + values.get()).forEach { part ->
            val bytes = part.toByteArray()
            digest.update(bytes.size.toString().toByteArray())
            digest.update(':'.code.toByte())
            digest.update(bytes)
        }
        return digest.digest().toHex()
    }

    private fun ByteArray.toHex() = joinToString("") { "%02x".format(it) }
}

internal abstract class StagedModule {
    @get:Input abstract val name: Property<String>

    @get:Input abstract val uses: ListProperty<String>

    @get:Nested abstract val contents: ListProperty<StagedFiles>

    @get:Nested @get:Optional abstract val source: Property<ModuleSource>
}

/** Stages the bundle: the manifest, and each module's files, listed by module in [MODULE_LIST]. */
internal abstract class StageBundleTask @Inject constructor(private val fs: FileSystemOperations) :
    DefaultTask() {
    @get:InputFile
    @get:PathSensitive(PathSensitivity.NONE)
    abstract val manifest: RegularFileProperty

    @get:Nested abstract val modules: ListProperty<StagedModule>

    @get:OutputDirectory abstract val output: DirectoryProperty

    @TaskAction
    fun stage() {
        val staged = modules.get().map { it to sortedSetOf<String>() }
        fs.sync {
            into(output)
            duplicatesStrategy = DuplicatesStrategy.FAIL
            from(manifest)
            staged.forEach { (module, files) ->
                module.contents.get().forEach { content ->
                    from(content.from) {
                        content.include.orNull?.let { include(it) }
                        into(content.into.get())
                        content.dexName.orNull?.let { name ->
                            rename { it.replace("classes", name) }
                        }
                        eachFile { files += path }
                    }
                }
            }
        }
        output
            .file(MODULE_LIST)
            .get()
            .asFile
            .writeText(
                bundleJson.encodeToString(
                    staged.map { (module, files) ->
                        ModuleListEntry(
                            module.name.get(),
                            files.toList(),
                            module.uses.get().sorted(),
                            module.source.orNull?.digest(),
                        )
                    }
                )
            )
    }
}
