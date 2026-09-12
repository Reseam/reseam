// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.gradle

import groovy.json.JsonSlurper
import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.provider.ListProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.InputFiles
import org.gradle.api.tasks.Internal
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.Serializable
import java.net.URI
import javax.inject.Inject

/** A released bundle, located through its `patches.json`. */
data class PublishedBundle(val index: String, val version: String, val signer: String?) : Serializable

/** Bundles whose patches this module may depend on; each one yields an `ExternalPatch` per patch, in its original package. */
abstract class ReseamPatchesExtension {
    internal val published = mutableListOf<PublishedBundle>()
    internal val local = mutableListOf<File>()

    /** `signer` pins the public key the index must carry. */
    fun bundle(index: String, version: String, signer: String? = null) {
        published += PublishedBundle(index, version, signer)
    }

    fun bundle(file: File) {
        local += file
    }
}

private class Listing(val name: String, val publicKey: String, val trusted: Boolean, val patches: List<String>)

abstract class GeneratePatchRefsTask @Inject constructor(private val exec: ExecOperations) : DefaultTask() {
    @get:Input
    abstract val published: ListProperty<PublishedBundle>

    @get:InputFiles
    abstract val local: ConfigurableFileCollection

    @get:Input
    abstract val reseamBinary: Property<String>

    @get:Internal
    abstract val cache: DirectoryProperty

    @get:OutputDirectory
    abstract val output: DirectoryProperty

    @TaskAction
    fun run() {
        val outDir = output.get().asFile
        outDir.deleteRecursively()
        outDir.mkdirs()
        for (bundle in published.get()) {
            val (file, signer) = download(bundle)
            write(outDir, list(file, signer))
        }
        for (file in local.files) {
            write(outDir, list(file, list(file, null).publicKey))
        }
    }

    private fun download(bundle: PublishedBundle): Pair<File, String> {
        val index = JsonSlurper().parse(URI(bundle.index).toURL()) as Map<*, *>
        val info = index["bundle"] as Map<*, *>
        val signer = info["public_key"] as String
        if (bundle.signer != null && bundle.signer != signer) {
            throw GradleException("${bundle.index} is signed by $signer, not the pinned ${bundle.signer}")
        }
        val releases = (index["releases"] as List<*>).map { it as Map<*, *> }
        val wanted = bundle.version.removePrefix("v")
        val release = releases.firstOrNull { (it["version"] as String).removePrefix("v") == wanted }
            ?: throw GradleException("${bundle.index} has no release $wanted; available: ${releases.joinToString { it["version"] as String }}")
        val file = cache.get().asFile.resolve("${info["name"]}-$wanted.reseam")
        if (!file.isFile) {
            file.parentFile.mkdirs()
            URI(release["download_url"] as String).toURL().openStream().use { stream -> file.outputStream().use(stream::copyTo) }
        }
        return file to signer
    }

    private fun list(file: File, trust: String?): Listing {
        val stdout = ByteArrayOutputStream()
        exec.exec {
            commandLine(listOfNotNull(reseamBinary.get(), "bundle", "list", file.absolutePath, "--json") + (trust?.let { listOf("--trust", it) } ?: emptyList()))
            standardOutput = stdout
        }
        val response = JsonSlurper().parseText(stdout.toString(Charsets.UTF_8)) as Map<*, *>
        val info = (response["bundles"] as List<*>).single() as Map<*, *>
        val patches = (response["patches"] as List<*>).map { (it as Map<*, *>)["id"] as String }
        return Listing(info["name"] as String, info["public_key"] as String, info["trusted"] as Boolean, patches)
    }

    private fun write(outDir: File, listing: Listing) {
        check(listing.trusted) { "${listing.name} could not be listed; its patches are only visible for a trusted signer" }
        for ((pkg, ids) in listing.patches.groupBy { it.substringBeforeLast('.', "") }) {
            val file = outDir.resolve(listing.name).resolve(pkg.replace('.', '/')).resolve("PatchRefs.kt")
            file.parentFile.mkdirs()
            file.writeText(
                buildString {
                    if (pkg.isNotEmpty()) append("package $pkg\n\n")
                    append("import app.reseam.patch.ExternalPatch\n\n")
                    for (id in ids) append("val ${id.substringAfterLast('.')} = ExternalPatch(\"${listing.name}\", \"$id\")\n")
                },
            )
        }
    }
}
