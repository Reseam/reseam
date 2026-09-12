// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.gradle

import org.gradle.api.GradleException
import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.api.file.DuplicatesStrategy
import org.gradle.api.tasks.Copy
import org.gradle.api.tasks.Exec
import org.gradle.api.tasks.Sync
import org.gradle.api.tasks.bundling.Jar
import java.io.File

/**
 * The root of a bundle: stages every patch jar and extension DEX next to
 * `manifest.toml` and packs them into a signed `.reseam` with the engine CLI.
 */
class ReseamBundlePlugin : Plugin<Project> {
    override fun apply(project: Project) {
        val manifest = project.layout.projectDirectory.file("manifest.toml")
        val bundleName = project.provider { bundleName(manifest.asFile) }
        val reseamBin = project.reseamBinary()
        val stageDir = project.layout.buildDirectory.dir("reseam/stage")
        val bundleFile = project.layout.buildDirectory.file(bundleName.map { "reseam/$it.reseam" })

        val patchJars = project.provider {
            project.subprojects.filter { it.plugins.hasPlugin(ReseamPatchesPlugin::class.java) }
                .map { it.tasks.named("patchJar", Jar::class.java) }
        }
        val extensionDex = project.provider {
            project.subprojects.filter { it.plugins.hasPlugin(ReseamExtensionPlugin::class.java) }
                .map { it.tasks.named("dex", DexTask::class.java) }
        }

        val stage = project.tasks.register("stageBundle", Copy::class.java) {
            group = "build"
            description = "Collects manifest.toml, patch jars, and extension DEX files."
            into(stageDir)
            doFirst { stageDir.get().asFile.deleteRecursively() }
            from(manifest)
            from(patchJars.map { jars -> jars.map { jar -> jar.map { it.archiveFile } } })
            extensionDex.get().forEach { dexTask ->
                dependsOn(dexTask)
                from(dexTask.map { it.output }) {
                    include("classes*.dex")
                    rename { name -> name.replace("classes", dexTask.get().project.reseamArtifactName()) }
                }
            }
            duplicatesStrategy = DuplicatesStrategy.FAIL
        }

        val bundle = project.tasks.register("bundle", Exec::class.java) {
            group = "build"
            description = "Packs the staged bundle into a signed .reseam file."
            dependsOn(stage)
            val signingKey = project.providers.environmentVariable("RESEAM_BUNDLE_KEY")
                .orElse(project.providers.gradleProperty("reseam.signingKey"))
                .orElse("${System.getProperty("user.home")}/.reseam/bundle-signing.key")
            inputs.dir(stageDir)
            outputs.file(bundleFile)
            doFirst {
                bundleFile.get().asFile.parentFile.mkdirs()
                commandLine(
                    reseamBin.get(), "bundle", "pack", stageDir.get().asFile.absolutePath,
                    "--key", signingKey.get(),
                    "--out", bundleFile.get().asFile.absolutePath,
                )
            }
        }

        val patchesJson = project.tasks.register("generatePatchesJson", Exec::class.java) {
            group = "distribution"
            description = "Writes the patches.json release index for the bundle."
            dependsOn(bundle)
            val env = { name: String -> project.providers.environmentVariable(name) }
            val releaseTag = project.providers.gradleProperty("releaseTag")
            val version = env("RESEAM_RELEASE_VERSION").orElse(releaseTag.map { it.removePrefix("v") })
            val url = env("RESEAM_BUNDLE_URL").orElse(releaseTag.zip(bundleName) { tag, name -> "https://api.reseam.app/patches/$tag/$name.reseam" })
            val outFile = env("RESEAM_PATCHES_JSON_OUT").map(project::file)
                .orElse(project.layout.buildDirectory.file("reseam/patches.json").map { it.asFile })
            inputs.file(bundleFile)
            outputs.file(outFile)
            doFirst {
                val args = mutableListOf(
                    reseamBin.get(), "publish", "patches", bundleFile.get().asFile.absolutePath,
                    "--version", version.orNull ?: throw GradleException("RESEAM_RELEASE_VERSION or -PreleaseTag is required"),
                    "--url", url.orNull ?: throw GradleException("RESEAM_BUNDLE_URL or -PreleaseTag is required"),
                    "--homepage", env("RESEAM_HOMEPAGE").getOrElse("https://reseam.app"),
                    "--out", outFile.get().absolutePath,
                )
                env("RESEAM_RELEASE_DESCRIPTION").orNull?.let { args += listOf("--description", it) }
                env("RESEAM_RELEASE_DESCRIPTION_FILE").orNull?.let { args += listOf("--description-file", project.file(it).absolutePath) }
                env("RESEAM_RELEASE_CREATED_AT").orNull?.let { args += listOf("--created-at", it) }
                if (env("RESEAM_RELEASE_PRERELEASE").map { it.equals("true", true) || it == "1" }.getOrElse(false)) args += "--prerelease"
                outFile.get().parentFile.mkdirs()
                commandLine(args)
            }
        }

        project.tasks.register("stageRelease", Sync::class.java) {
            group = "distribution"
            description = "Collects the bundle and patches.json for a release."
            dependsOn(patchesJson)
            from(bundleFile)
            from(patchesJson.map { it.outputs.files })
            into(project.layout.buildDirectory.dir("reseam/release"))
        }
    }

    private fun bundleName(manifest: File): String {
        if (!manifest.isFile) throw GradleException("manifest.toml is missing from ${manifest.parentFile}")
        return Regex("""^\s*name\s*=\s*"([^"]+)"""", RegexOption.MULTILINE).find(manifest.readText())?.groupValues?.get(1)
            ?: throw GradleException("manifest.toml declares no bundle name")
    }
}
