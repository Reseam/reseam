// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.io.File
import javax.inject.Inject
import kotlinx.serialization.Serializable
import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.Project
import org.gradle.api.file.ArchiveOperations
import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.FileCollection
import org.gradle.api.file.FileSystemOperations
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.provider.ListProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.InputFiles
import org.gradle.api.tasks.Internal
import org.gradle.api.tasks.Nested
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.OutputFile
import org.gradle.api.tasks.PathSensitive
import org.gradle.api.tasks.PathSensitivity
import org.gradle.api.tasks.TaskAction
import org.gradle.api.tasks.TaskProvider
import org.gradle.process.ExecOperations

/** The pinned TypeScript compiler, published as one npm package per host. */
internal object TypeScript {
    const val SCOPE = "typescript"

    fun tool() =
        when (Host.current) {
            Host.LINUX_X64 ->
                build(
                    "linux-x64",
                    "sha512-EYdf2cNg7rgCWJnxCdJ+F3V39O8ihb37eHAu1LK8oAFizgTQbPOK7zHHXbPt8rX24COqODXeI3sIf0fCXG7H/A==",
                    "lib/tsc",
                )
            Host.MACOS_X64 ->
                build(
                    "darwin-x64",
                    "sha512-SZ9xZInqApNlNGc9s0W1VSsktYSOe9cFqNOIqmN1Gs8SmkjKZYFt017G4VwPxASInODuAdbTW7sXiFUf893RgA==",
                    "lib/tsc",
                )
            Host.MACOS_ARM64 ->
                build(
                    "darwin-arm64",
                    "sha512-gowzar9MwS/aRWp6f3a4KUqzRjAZjOsmGNCM6LcTgXum+dBfgsBVMN+AgvOCCbguXyick6LJhpBszxMebJ8syA==",
                    "lib/tsc",
                )
            Host.WINDOWS_X64 ->
                build(
                    "win32-x64",
                    "sha512-0BQ3HkAHHlKLSp1qRvf3SUhGpGsDuhB/jgFw75guyqbxJqEaS0Cw/VFO8i2nHglJUzQCRtMMR/IBAKE3ETMC4g==",
                    "lib/tsc.exe",
                )
        }

    private fun build(platform: String, integrity: String, executable: String) =
        NpmTool(NpmPackage("@$SCOPE/typescript-$platform", "7.0.2"), integrity, executable)
}

@Serializable private data class PackageJson(val devDependencies: Map<String, String> = emptyMap())

@Serializable private data class PackageLock(val packages: Map<String, LockedPackage>)

@Serializable
private data class LockedPackage(val version: String? = null, val integrity: String? = null)

/** A type package to install: its name, its integrity and its tarball. */
internal abstract class TypePackage {
    @get:Input abstract val name: Property<String>
    @get:Input abstract val integrity: Property<String>

    @get:InputFiles
    @get:PathSensitive(PathSensitivity.NONE)
    abstract val archive: ConfigurableFileCollection
}

/** Installs a folder's type packages into its `node_modules`, where TypeScript and editors look. */
internal abstract class TypePackagesTask
@Inject
constructor(private val archives: ArchiveOperations, private val files: FileSystemOperations) :
    DefaultTask() {
    @get:Nested abstract val packages: ListProperty<TypePackage>
    @get:OutputDirectory abstract val modules: DirectoryProperty

    @TaskAction
    fun install() {
        val root = modules.get().asFile.also(::recreate)
        for (pkg in packages.get()) {
            val archive = pkg.archive.singleFile
            verifyIntegrity(archive, pkg.integrity.get())
            files.copy {
                from(archives.tarTree(archive)) {
                    include("*/**/*.d.ts", "*/**/package.json")
                    eachFile { relativePath = relativePath.inPackage() }
                }
                into(File(root, pkg.name.get()))
                includeEmptyDirs = false
            }
        }
    }
}

/** Checks a folder's TypeScript with its `tsconfig.json`. */
internal abstract class TypecheckTask
@Inject
constructor(
    private val exec: ExecOperations,
    private val archives: ArchiveOperations,
    private val files: FileSystemOperations,
) : DefaultTask() {
    @get:InputFiles
    @get:PathSensitive(PathSensitivity.NONE)
    val compiler: FileCollection = project.npmArchive { TypeScript.tool().pkg }

    @get:InputFiles
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val sources: ConfigurableFileCollection
    @get:Internal abstract val config: RegularFileProperty
    @get:OutputFile abstract val buildInfo: RegularFileProperty

    @TaskAction
    fun check() {
        val tsc =
            TypeScript.tool()
                .unpack(compiler.singleFile, File(temporaryDir, "typescript"), archives, files)
        exec.exec {
            commandLine(
                tsc.absolutePath,
                "--project",
                config.get().asFile.absolutePath,
                "--incremental",
                "--tsBuildInfoFile",
                buildInfo.get().asFile.absolutePath,
            )
        }
    }
}

/** The type packages of a typed [folder] with their integrity; see [registerTypecheck]. */
internal fun typePackages(folder: File): List<Pair<NpmPackage, String>> {
    val manifest = bundleJson.decodeFromString<PackageJson>(File(folder, "package.json").readText())
    val lock =
        bundleJson.decodeFromString<PackageLock>(File(folder, "package-lock.json").readText())
    return manifest.devDependencies.map { (pkg, version) ->
        val entry =
            lock.packages["node_modules/$pkg"]
                ?: throw GradleException("$folder: $pkg is missing from package-lock.json")
        if (entry.version != version)
            throw GradleException(
                "$folder: package.json pins $pkg $version, package-lock.json has ${entry.version}"
            )
        NpmPackage(pkg, version) to
            (entry.integrity
                ?: throw GradleException("$folder: package-lock.json has no integrity for $pkg"))
    }
}

/**
 * Registers the TypeScript check of a workspace [folder] whose extensions are typed: its
 * `package.json` lists the type packages they use as `devDependencies` at exact versions,
 * `package-lock.json` holds their integrity, and `tsconfig.json` configures the check. Only type
 * declarations are installed, so every package the declarations import is listed too. The React
 * Native modules of its Kotlin [extensions] are declared from their classes; `tsconfig.json`
 * includes them from each extension's `build/reseam/types`.
 */
internal fun Project.registerTypecheck(
    folder: File,
    name: String,
    extensions: List<Project>,
): TaskProvider<TypecheckTask> {
    val locked = typePackages(folder)
    val install =
        tasks.register("installTypes-$name", TypePackagesTask::class.java) {
            group = "build"
            description = "Installs the type packages of $name into its node_modules."
            locked.forEach { (pkg, integrity) ->
                packages.add(
                    objects.newInstance(TypePackage::class.java).also {
                        it.name.set(pkg.name)
                        it.integrity.set(integrity)
                        it.archive.from(npmArchive { pkg })
                    }
                )
            }
            modules.set(File(folder, "node_modules"))
        }
    return tasks.register("typecheck-$name", TypecheckTask::class.java) {
        group = "verification"
        description = "Checks the TypeScript of $name."
        config.set(File(folder, "tsconfig.json"))
        sources.from(
            fileTree(folder) {
                include("tsconfig.json", "**/*.ts", "**/*.tsx")
                exclude("**/build/**", "node_modules/**")
            },
            install.flatMap { it.modules },
            extensions.map { it.layout.buildDirectory.dir("reseam/types") },
        )
        dependsOn(extensions.map { "${it.path}:$NATIVE_MODULE_TYPES" })
        buildInfo.set(layout.buildDirectory.file("reseam/typescript/$name.tsbuildinfo"))
    }
}
