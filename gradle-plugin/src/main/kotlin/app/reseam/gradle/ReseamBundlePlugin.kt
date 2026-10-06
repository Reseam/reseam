// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import org.gradle.api.GradleException
import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.api.file.DuplicatesStrategy
import org.gradle.api.tasks.Exec
import org.gradle.api.tasks.Sync
import org.gradle.api.tasks.bundling.Jar

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
                .map { bundleJson.decodeFromString<BundleMetadata>(it).name }
        val stageDir = project.layout.buildDirectory.dir("reseam/stage")
        val bundleFile = project.layout.buildDirectory.file(bundleName.map { "reseam/$it.reseam" })

        val patchJars = project.provider {
            project.subprojects
                .filter { it.plugins.hasPlugin(ReseamPatchesPlugin::class.java) }
                .map { it.tasks.named("patchJar", Jar::class.java) }
        }
        val extensionDex = project.provider {
            project.subprojects
                .filter {
                    it.plugins.hasPlugin(ReseamExtensionPlugin::class.java) &&
                        !it.plugins.hasPlugin(ReseamJavascriptPlugin::class.java)
                }
                .map { it.tasks.named("dex", DexTask::class.java) }
        }
        val extensionHbc = project.provider {
            project.subprojects
                .filter { it.plugins.hasPlugin(ReseamJavascriptPlugin::class.java) }
                .map { it.tasks.named(HermesCompiler.TASK, HermesCompileTask::class.java) }
        }

        val globalsDex =
            project.tasks.register("dexGlobalSynthetics", GlobalSyntheticsDexTask::class.java) {
                description = "Compiles the extensions' d8 global synthetics into one shared DEX."
                extensionDex.get().forEach { dexTask ->
                    globals.from(dexTask.flatMap { it.globals }.map { it.file("classes.globals") })
                    dependsOn(dexTask)
                }
                libraries.from(project.provider { AndroidSdk.platformJar() })
                output.set(project.layout.buildDirectory.dir("reseam/globals-dex"))
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
                extensionDex.get().forEach { dexTask ->
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
                from(globalsDex.map { it.output }) {
                    include("classes*.dex")
                    rename { name -> name.replace("classes", "d8-globals") }
                }
                duplicatesStrategy = DuplicatesStrategy.FAIL
            }

        val bundle =
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
                outputs.file(bundleFile)
                doFirst {
                    bundleFile.get().asFile.parentFile.mkdirs()
                    commandLine(
                        reseamBin.get().absolutePath,
                        "bundle",
                        "pack",
                        stageDir.get().asFile.absolutePath,
                        "--key",
                        signingKey.get(),
                        "--out",
                        bundleFile.get().asFile.absolutePath,
                    )
                }
            }

        val patchesJson =
            project.tasks.register("generatePatchesJson", Exec::class.java) {
                group = "distribution"
                description = "Writes the patches.json release index for the bundle."
                dependsOn(bundle)
                val env = { name: String -> project.providers.environmentVariable(name) }
                val releaseTag = project.providers.gradleProperty("releaseTag")
                val version =
                    env("RESEAM_RELEASE_VERSION").orElse(releaseTag.map { it.removePrefix("v") })
                val url =
                    env("RESEAM_BUNDLE_URL")
                        .orElse(
                            releaseTag.zip(bundleName) { tag, name ->
                                "https://api.reseam.app/patches/$tag/$name.reseam"
                            }
                        )
                val outFile =
                    env("RESEAM_PATCHES_JSON_OUT")
                        .map(project::file)
                        .orElse(
                            project.layout.buildDirectory.file("reseam/patches.json").map {
                                it.asFile
                            }
                        )
                val homepage = env("RESEAM_HOMEPAGE").orElse("https://reseam.app")
                val description = env("RESEAM_RELEASE_DESCRIPTION").orElse("")
                val descriptionFile = env("RESEAM_RELEASE_DESCRIPTION_FILE").map(project::file)
                val createdAt = env("RESEAM_RELEASE_CREATED_AT").orElse("")
                val prerelease =
                    env("RESEAM_RELEASE_PRERELEASE")
                        .map { it.equals("true", true) || it == "1" }
                        .orElse(false)
                inputs.file(bundleFile)
                inputs.file(reseamBin)
                inputs.property("version", version.orElse(""))
                inputs.property("url", url.orElse(""))
                inputs.property("homepage", homepage)
                inputs.property("description", description)
                inputs.file(descriptionFile).optional()
                inputs.property("createdAt", createdAt)
                inputs.property("prerelease", prerelease)
                outputs.upToDateWhen { false }
                outputs.file(outFile)
                doFirst {
                    val args =
                        mutableListOf(
                            reseamBin.get().absolutePath,
                            "publish",
                            "patches",
                            bundleFile.get().asFile.absolutePath,
                            "--version",
                            version.orNull
                                ?: throw GradleException(
                                    "RESEAM_RELEASE_VERSION or -PreleaseTag is required"
                                ),
                            "--url",
                            url.orNull
                                ?: throw GradleException(
                                    "RESEAM_BUNDLE_URL or -PreleaseTag is required"
                                ),
                            "--homepage",
                            homepage.get(),
                            "--out",
                            outFile.get().absolutePath,
                        )
                    description
                        .get()
                        .takeIf { it.isNotEmpty() }
                        ?.let {
                            args += listOf("--description", it)
                        }
                    descriptionFile.orNull?.let {
                        args += listOf("--description-file", it.absolutePath)
                    }
                    createdAt
                        .get()
                        .takeIf { it.isNotEmpty() }
                        ?.let {
                            args += listOf("--created-at", it)
                        }
                    if (prerelease.get()) args += "--prerelease"
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
}
