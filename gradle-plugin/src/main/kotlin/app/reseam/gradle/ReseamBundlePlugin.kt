// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import kotlinx.serialization.Serializable
import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.api.file.RegularFile
import org.gradle.api.provider.Provider
import org.gradle.api.tasks.Exec

/** What `reseam bundle manifest` prints, as far as the build reads it. */
@Serializable private data class ManifestInfo(val name: String)

internal class ReseamBundlePlugin : Plugin<Project> {
    override fun apply(project: Project) {
        val manifest = project.layout.projectDirectory.file("manifest.toml")
        val reseamBin = project.reseamExecutable()
        val bundleName =
            project.providers
                .exec {
                    commandLine(
                        reseamBin.get().absolutePath,
                        "bundle",
                        "manifest",
                        manifest.asFile.absolutePath,
                    )
                }
                .standardOutput
                .asText
                .map { bundleJson.decodeFromString<ManifestInfo>(it).name }
        project.extensions.create("reseamBundle", ReseamBundleInfo::class.java).name.set(bundleName)
        val version =
            project.providers
                .environmentVariable("RESEAM_RELEASE_VERSION")
                .orElse(project.providers.gradleProperty("releaseTag").map { it.removePrefix("v") })
        val stageDir = project.layout.buildDirectory.dir("reseam/stage")
        val bundleFile = project.layout.buildDirectory.file(bundleName.map { "reseam/$it.reseam" })

        val dex = project.registerBundleDex()
        val stage =
            project.tasks.register("stageBundle", StageBundleTask::class.java) {
                group = "build"
                description =
                    "Collects the manifest and each module's patch jars, DEX and resources, and lists them by module."
                this.manifest.set(manifest)
                modules.set(project.bundleModules(dex))
                output.set(stageDir)
            }

        fun registerPack(
            name: String,
            description: String,
            output: Provider<RegularFile>,
            keyArguments: Provider<List<String>>,
        ) =
            project.tasks.register(name, Exec::class.java) {
                group = "build"
                this.description = description
                dependsOn(stage)
                inputs.dir(stageDir)
                inputs.file(reseamBin)
                inputs.property("version", version.orElse(""))
                outputs.file(output)
                doFirst {
                    output.get().asFile.parentFile.mkdirs()
                    commandLine(
                        listOf(
                            reseamBin.get().absolutePath,
                            "bundle",
                            "pack",
                            stageDir.get().asFile.absolutePath,
                            "--out",
                            output.get().asFile.absolutePath,
                        ) +
                            keyArguments.get() +
                            version.map { listOf("--version", it) }.getOrElse(emptyList())
                    )
                }
            }

        val signingKey =
            project.providers
                .environmentVariable("RESEAM_BUNDLE_KEY")
                .orElse(project.providers.gradleProperty("reseam.signingKey"))
                .orElse("${System.getProperty("user.home")}/.reseam/bundle-signing.key")
        registerPack(
                "bundle",
                "Packs the staged bundle into a signed .reseam file.",
                bundleFile,
                signingKey.map { listOf("--key", it) },
            )
            .configure { inputs.file(signingKey.map(project::file)) }
        registerPack(
            "unsignedBundle",
            "Packs the staged bundle into an unsigned .reseam file, for `reseam bundle sign` to sign elsewhere.",
            project.layout.buildDirectory.file(bundleName.map { "reseam/unsigned/$it.reseam" }),
            project.provider { emptyList() },
        )
    }
}
