// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import kotlinx.serialization.Serializable
import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.api.file.DuplicatesStrategy
import org.gradle.api.tasks.Exec
import org.gradle.api.tasks.Sync
import org.gradle.api.tasks.bundling.Jar

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

        val patchJars = project.provider {
            project.subprojects
                .filter { it.plugins.hasPlugin(ReseamPatchesPlugin::class.java) }
                .map { it.tasks.named("patchJar", Jar::class.java) }
        }
        val dex = project.registerBundleDex()
        val extensionHbc = project.provider {
            project.subprojects
                .filter { it.plugins.hasPlugin(ReseamJavascriptPlugin::class.java) }
                .map { it.tasks.named(HermesCompiler.TASK, HermesCompileTask::class.java) }
        }

        val stage =
            project.tasks.register("stageBundle", Sync::class.java) {
                group = "build"
                description = "Collects the manifest, patch jars, extension DEX, and resources."
                into(stageDir)
                from(manifest)
                from(project.layout.projectDirectory.dir("resources")) { into("resources") }
                extensionHbc.get().forEach { task ->
                    dependsOn(task)
                    from(task.flatMap { it.output }) {
                        include("*.hbc")
                        into("resources/hermes/v${HermesCompiler.BYTECODE_VERSION}")
                    }
                }
                from(patchJars.map { jars -> jars.map { jar -> jar.map { it.archiveFile } } })
                dex.modules.get().forEach { dexTask ->
                    dependsOn(dexTask)
                    from(dexTask.map { it.output }) {
                        include("classes*.dex")
                        rename { name ->
                            name.replace(
                                "classes",
                                dexTask.get().project.reseamArtifact().name.get(),
                            )
                        }
                    }
                }
                from(dex.kotlin.map { it.output }) {
                    include("classes*.dex")
                    rename { name -> name.replace("classes", "kotlin") }
                }
                from(dex.globals.map { it.output }) {
                    include("classes*.dex")
                    rename { name -> name.replace("classes", "d8-globals") }
                }
                duplicatesStrategy = DuplicatesStrategy.FAIL
            }

        project.tasks.register("bundle", Exec::class.java) {
            group = "build"
            description = "Packs the staged bundle into a signed .reseam file."
            dependsOn(stage)
            val signingKey =
                project.providers
                    .environmentVariable("RESEAM_BUNDLE_KEY")
                    .orElse(project.providers.gradleProperty("reseam.signingKey"))
                    .orElse("${System.getProperty("user.home")}/.reseam/bundle-signing.key")
            inputs.dir(stageDir)
            inputs.file(reseamBin)
            inputs.file(signingKey.map(project::file))
            inputs.property("version", version.orElse(""))
            outputs.file(bundleFile)
            doFirst {
                bundleFile.get().asFile.parentFile.mkdirs()
                commandLine(
                    listOf(
                        reseamBin.get().absolutePath,
                        "bundle",
                        "pack",
                        stageDir.get().asFile.absolutePath,
                        "--key",
                        signingKey.get(),
                        "--out",
                        bundleFile.get().asFile.absolutePath,
                    ) + version.map { listOf("--version", it) }.getOrElse(emptyList())
                )
            }
        }
    }
}
