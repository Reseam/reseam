// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import java.io.OutputStream
import java.net.URI
import java.security.DigestInputStream
import java.security.MessageDigest
import java.util.Base64
import org.gradle.api.GradleException
import org.gradle.api.Project
import org.gradle.api.artifacts.dsl.RepositoryHandler
import org.gradle.api.artifacts.repositories.RepositoryContentDescriptor
import org.gradle.api.file.ArchiveOperations
import org.gradle.api.file.FileCollection
import org.gradle.api.file.FileSystemOperations
import org.gradle.api.file.RelativePath

/** The host a build runs on, as the pinned native tools are published for it. */
internal enum class Host {
    LINUX_X64,
    MACOS_X64,
    MACOS_ARM64,
    WINDOWS_X64;

    companion object {
        val current: Host by lazy {
            val os = System.getProperty("os.name")
            val arch = System.getProperty("os.arch")
            val arm = arch == "aarch64" || arch == "arm64"
            when {
                // Windows on Arm runs the x64 builds under emulation.
                os.startsWith("Windows") -> WINDOWS_X64
                os.startsWith("Mac") -> if (arm) MACOS_ARM64 else MACOS_X64
                os.startsWith("Linux") && !arm -> LINUX_X64
                else -> throw GradleException("JavaScript extensions cannot be built on $os $arch")
            }
        }
    }
}

/** A package version on npm's registry, by its full name, such as `@types/react`. */
internal data class NpmPackage(val name: String, val version: String) {
    /** The Gradle notation [npmRegistry] resolves to the package's tarball. */
    val dependency: String
        get() {
            val module = name.substringAfter('/')
            val group = if (name.startsWith("@")) name.substringBefore('/').drop(1) else module
            return "$group:$module:$version@tgz"
        }
}

/** A native executable published to npm, pinned by the registry's integrity string. */
internal class NpmTool(
    val pkg: NpmPackage,
    private val integrity: String,
    private val executable: String,
) {
    /** Checks and unpacks [archive] into [directory] and returns the executable. */
    fun unpack(
        archive: File,
        directory: File,
        archives: ArchiveOperations,
        files: FileSystemOperations,
    ): File {
        verifyIntegrity(archive, integrity)
        files.sync {
            from(archives.tarTree(archive)) { eachFile { relativePath = relativePath.inPackage() } }
            into(directory)
            includeEmptyDirs = false
        }
        return File(directory, executable).also {
            if (!it.setExecutable(true) && !it.canExecute())
                throw GradleException("cannot execute $it")
        }
    }
}

/** The archive of [pkg], chosen and resolved when first read. */
internal fun Project.npmArchive(pkg: () -> NpmPackage): FileCollection =
    objects
        .fileCollection()
        .from(
            provider { configurations.detachedConfiguration(dependencies.create(pkg().dependency)) }
        )

/** A path inside an npm tarball, relative to the package; the tarball's top folder is dropped. */
internal fun RelativePath.inPackage() = RelativePath(isFile, *segments.drop(1).toTypedArray())

/**
 * Checks [file] against an npm integrity string, such as `sha512-…`. Of several hashes, the first
 * is checked.
 */
internal fun verifyIntegrity(file: File, integrity: String) {
    val (algorithm, expected) = integrity.substringBefore(' ').split('-', limit = 2)
    val digest =
        MessageDigest.getInstance(
            when (algorithm) {
                "sha512" -> "SHA-512"
                "sha384" -> "SHA-384"
                "sha256" -> "SHA-256"
                else -> throw GradleException("unsupported integrity algorithm $algorithm")
            }
        )
    DigestInputStream(file.inputStream(), digest).use { it.copyTo(OutputStream.nullOutputStream()) }
    if (Base64.getEncoder().encodeToString(digest.digest()) != expected)
        throw GradleException("${file.name} does not match its integrity $integrity")
}

/**
 * npm's registry as Ivy repositories for [packages], by full name such as `@types/react`, and for
 * every package of [scopes].
 */
internal fun RepositoryHandler.npmRegistry(
    packages: Set<String> = emptySet(),
    scopes: Set<String> = emptySet(),
) {
    val (scoped, unscoped) = packages.partition { it.startsWith("@") }
    if (unscoped.isNotEmpty())
        npmRepository("[organisation]/-/[module]-[revision].[ext]") {
            unscoped.forEach { includeModule(it, it) }
        }
    if (scoped.isNotEmpty() || scopes.isNotEmpty())
        npmRepository("@[organisation]/[module]/-/[module]-[revision].[ext]") {
            scopes.forEach { includeGroup(it) }
            scoped.forEach {
                includeModule(it.substringBefore('/').drop(1), it.substringAfter('/'))
            }
        }
}

private fun RepositoryHandler.npmRepository(
    pattern: String,
    content: RepositoryContentDescriptor.() -> Unit,
) {
    ivy {
        url = URI("https://registry.npmjs.org")
        patternLayout { artifact(pattern) }
        metadataSources { artifact() }
        content(content)
    }
}
