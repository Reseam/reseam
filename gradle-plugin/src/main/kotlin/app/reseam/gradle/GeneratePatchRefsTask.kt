// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.ByteArrayOutputStream
import java.io.File
import javax.inject.Inject
import kotlinx.serialization.Serializable
import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.provider.ListProperty
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.InputFile
import org.gradle.api.tasks.InputFiles
import org.gradle.api.tasks.Internal
import org.gradle.api.tasks.Optional
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations

@Serializable private data class ListedBundle(val name: String, val problem: BundleProblem? = null)

@Serializable private data class BundleProblem(val type: String)

@Serializable private data class ListedPatch(val id: String)

@Serializable
private data class BundleListing(val bundles: List<ListedBundle>, val patches: List<ListedPatch>)

/** Generates `ExternalPatch` declarations for the patches of the bundles a module references. */
internal abstract class GeneratePatchRefsTask
@Inject
constructor(private val exec: ExecOperations) : DefaultTask() {
    @get:Input abstract val published: ListProperty<PublishedBundle>
    @get:InputFiles abstract val local: ConfigurableFileCollection
    @get:InputFile @get:Optional abstract val reseamBinary: RegularFileProperty
    @get:Internal abstract val cache: DirectoryProperty
    @get:OutputDirectory abstract val output: DirectoryProperty

    init {
        // Release indexes can change a version's download URL or signer.
        outputs.upToDateWhen { published.get().isEmpty() }
    }

    @TaskAction
    fun run() {
        val outDir = output.get().asFile.also(::recreate)
        val listings =
            published.get().map { bundle -> list(fetch(bundle)) } +
                local.files.sortedBy { it.path }.map(::list)
        if (listings.map { it.bundle }.toSet().size != listings.size)
            throw GradleException("referenced bundle names must be unique")
        listings.forEach { writePatchRefs(outDir, it) }
    }

    /** Downloads [bundle] into the cache, verified against its index signer, through the CLI. */
    private fun fetch(bundle: PublishedBundle): File {
        val signer = bundle.signer?.let { listOf("--signer", it) } ?: emptyList()
        val path =
            reseam(
                listOf("bundle", "fetch", "--index", bundle.index, "--version", bundle.version) +
                    signer +
                    listOf("--to", cache.get().asFile.absolutePath)
            )
        return File(path.trim())
    }

    private fun list(file: File): BundlePatches {
        val response =
            bundleJson.decodeFromString<BundleListing>(
                reseam(listOf("bundle", "list", file.absolutePath, "--json"))
            )
        val info = response.bundles.single()
        if (info.problem != null) throw GradleException("cannot inspect $file: ${info.problem}")
        return BundlePatches(info.name, response.patches.map { it.id })
    }

    private fun reseam(arguments: List<String>): String {
        val stdout = ByteArrayOutputStream()
        exec.exec {
            commandLine(listOf(reseamBinary.get().asFile.absolutePath) + arguments)
            standardOutput = stdout
        }
        return stdout.toString(Charsets.UTF_8)
    }
}
