// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.gradle

import java.io.ByteArrayOutputStream
import java.io.File
import java.io.Serializable as JavaSerializable
import java.net.URI
import java.nio.file.Files
import java.nio.file.StandardCopyOption.ATOMIC_MOVE
import java.nio.file.StandardCopyOption.REPLACE_EXISTING
import java.security.MessageDigest
import javax.inject.Inject
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.provider.ListProperty
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.InputFile
import org.gradle.api.tasks.InputFiles
import org.gradle.api.tasks.Internal
import org.gradle.api.tasks.Optional
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations

internal data class PublishedBundle(val index: String, val version: String, val signer: String?) :
    JavaSerializable

/** Bundles whose patches this module may reference through generated Kotlin declarations. */
abstract class ReseamPatchesExtension {
    internal val published = mutableListOf<PublishedBundle>()
    internal val local = mutableListOf<File>()

    fun bundle(index: String, version: String, signer: String? = null) {
        published += PublishedBundle(index, version, signer)
    }

    fun bundle(file: File) {
        local += file
    }
}

@Serializable
internal data class BundleMetadata(
    val name: String,
    @SerialName("public_key") val publicKey: String = "",
)

@Serializable
private data class Release(
    val version: String,
    @SerialName("download_url") val downloadUrl: String,
)

@Serializable
private data class ReleaseIndex(val bundle: BundleMetadata, val releases: List<Release>)

@Serializable
private data class ListedBundle(
    val name: String,
    @SerialName("public_key") val publicKey: String,
    val trusted: Boolean,
    val problem: BundleProblem? = null,
)

@Serializable private data class BundleProblem(val type: String)

@Serializable private data class ListedPatch(val id: String)

@Serializable
private data class BundleListing(val bundles: List<ListedBundle>, val patches: List<ListedPatch>)

@Serializable
private data class ReleaseIdentity(
    val source: String,
    val download: String,
    val version: String,
    val signer: String,
)

private data class Listing(val bundle: BundleMetadata, val patches: List<String>)

internal abstract class GeneratePatchRefsTask
@Inject
constructor(private val exec: ExecOperations) : DefaultTask() {
    @get:Input abstract val published: ListProperty<PublishedBundle>
    @get:InputFiles abstract val local: ConfigurableFileCollection
    @get:InputFile @get:Optional abstract val reseamBinary: org.gradle.api.file.RegularFileProperty
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
            published.get().map { bundle ->
                val source = URI(bundle.index).normalize()
                val index =
                    source.toURL().openStream().use {
                        bundleJson.decodeFromString<ReleaseIndex>(it.reader().readText())
                    }
                val publisher = index.bundle.copy(publicKey = index.bundle.publicKey.lowercase())
                if (
                    index.releases.distinctBy { it.version.removePrefix("v") }.size !=
                        index.releases.size
                )
                    throw GradleException("$source has duplicate release versions")
                if (bundle.signer != null && bundle.signer.lowercase() != publisher.publicKey)
                    throw GradleException("$source signer differs from pinned ${bundle.signer}")
                if (publisher.publicKey.isEmpty())
                    throw GradleException("$source has no bundle public key")
                val wanted = bundle.version.removePrefix("v")
                val release =
                    index.releases.singleOrNull { it.version.removePrefix("v") == wanted }
                        ?: throw GradleException("$source has no release $wanted")
                download(
                    source,
                    publisher,
                    source.resolve(release.downloadUrl).normalize(),
                    wanted,
                )
            } +
                local.files
                    .sortedBy { it.path }
                    .map { file -> list(file, list(file, null).bundle.publicKey) }
        if (listings.map { it.bundle.name }.toSet().size != listings.size)
            throw GradleException("referenced bundle names must be unique")
        listings.forEach { write(outDir, it) }
    }

    private fun download(
        source: URI,
        bundle: BundleMetadata,
        download: URI,
        version: String,
    ): Listing {
        val identity =
            bundleJson.encodeToString(
                ReleaseIdentity(
                    source.toASCIIString(),
                    download.toASCIIString(),
                    version,
                    bundle.publicKey,
                )
            )
        val digest =
            MessageDigest.getInstance("SHA-256").digest(identity.toByteArray(Charsets.UTF_8))
        val key = java.util.HexFormat.of().formatHex(digest)
        val dir = cache.get().asFile.toPath()
        Files.createDirectories(dir)
        val file = dir.resolve("$key.reseam")
        val temporary =
            if (Files.isRegularFile(file)) null else Files.createTempFile(dir, "$key-", ".download")
        try {
            if (temporary != null) {
                download.toURL().openStream().use { stream ->
                    Files.newOutputStream(temporary).use(stream::copyTo)
                }
            }
            val listing = list((temporary ?: file).toFile(), bundle.publicKey)
            if (listing.bundle != bundle)
                throw GradleException("${source} release identity differs from its signed bundle")
            if (temporary != null) {
                Files.move(temporary, file, ATOMIC_MOVE, REPLACE_EXISTING)
            }
            return listing
        } finally {
            if (temporary != null) Files.deleteIfExists(temporary)
        }
    }

    private fun list(file: File, trust: String?): Listing {
        val stdout = ByteArrayOutputStream()
        exec.exec {
            commandLine(
                listOf(
                    reseamBinary.get().asFile.absolutePath,
                    "bundle",
                    "list",
                    file.absolutePath,
                    "--json",
                ) + (trust?.let { listOf("--trust", it) } ?: emptyList())
            )
            standardOutput = stdout
        }
        val response = bundleJson.decodeFromString<BundleListing>(stdout.toString(Charsets.UTF_8))
        val info = response.bundles.single()
        if (info.problem != null) throw GradleException("cannot inspect $file: ${info.problem}")
        if (trust != null && (!info.trusted || info.publicKey != trust))
            throw GradleException("$file signer differs from pinned $trust")
        return Listing(BundleMetadata(info.name, info.publicKey), response.patches.map { it.id })
    }

    private fun write(outDir: File, listing: Listing) {
        for ((pkg, ids) in listing.patches.groupBy { it.substringBeforeLast('.', "") }) {
            val segments =
                pkg.split('.').filter(String::isNotEmpty) + ids.map { it.substringAfterLast('.') }
            if (
                segments.any {
                    it.contains('`') ||
                        it.contains('/') ||
                        it.contains('\\') ||
                        it.any(Char::isISOControl) ||
                        it in setOf(".", "..")
                }
            )
                throw GradleException("invalid Kotlin declaration in ${listing.bundle.name}")
            val file =
                outDir
                    .resolve(listing.bundle.name)
                    .resolve(pkg.replace('.', '/'))
                    .resolve("PatchRefs.kt")
            Files.createDirectories(file.parentFile.toPath())
            val literal = { value: String -> bundleJson.encodeToString(value).replace("$", "\\$") }
            file.writeText(
                buildString {
                    if (pkg.isNotEmpty())
                        append("package ${pkg.split('.').joinToString(".") { "`$it`" }}\n\n")
                    append("import app.reseam.patch.ExternalPatch\n\n")
                    for (id in ids) append(
                        "val `${id.substringAfterLast('.')}` = ExternalPatch(${literal(listing.bundle.name)}, ${literal(id)})\n"
                    )
                }
            )
        }
    }
}
