// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import org.gradle.api.GradleException
import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.api.tasks.JavaExec
import org.gradle.api.tasks.SourceSetContainer

/**
 * A module of settings groups and strings that the patches and the extensions of a bundle share.
 * The patches compile against its `main` source set; the extensions against objects the build
 * generates from it, with the same names, that read the stored values and translated strings.
 */
internal class ReseamSettingsPlugin : Plugin<Project> {
    override fun apply(project: Project) {
        if (!project.inBundle())
            throw GradleException("${project.path}: settings modules belong to a bundle")
        project.reseamArtifact()
        project.applyExtensionKotlin()
        project.dependencies.add("compileOnly", "app.reseam:reseam-patch-sdk:$reseamVersion")
        val sourceSets = project.extensions.getByType(SourceSetContainer::class.java)
        val main = sourceSets.getByName("main")
        val device = sourceSets.create("device")
        val strings =
            project.registerStrings()
                ?: throw GradleException(
                    "${project.path} has no src/main/strings/values/strings.xml"
                )
        project.kotlinSourceSet("main").kotlin.srcDir(strings.flatMap { it.host })
        main.resources.srcDir(strings.flatMap { it.json })

        val describe =
            project.tasks.register("describeSettings", JavaExec::class.java) {
                val output = project.layout.buildDirectory.file("reseam/settings.json")
                classpath = main.compileClasspath + main.output
                mainClass.set("app.reseam.patch.settings.SettingsDescriptionKt")
                inputs.files(main.output.classesDirs)
                outputs.file(output)
                argumentProviders.add {
                    listOf(output.get().asFile.absolutePath) +
                        main.output.classesDirs.map { it.absolutePath }
                }
            }
        val deviceSettings =
            project.tasks.register("generateDeviceSettings", DeviceSettingsTask::class.java) {
                groups.set(project.layout.buildDirectory.file("reseam/settings.json"))
                dependsOn(describe)
                bundle.set(project.bundleInfo().name)
                output.set(project.layout.buildDirectory.dir("generated/reseam/settings/device"))
            }
        project.kotlinSourceSet("device").kotlin.apply {
            srcDir(strings.flatMap { it.device })
            srcDir(deviceSettings.flatMap { it.output })
        }
        val platform = project.files(project.provider { AndroidSdk.platformJar() })
        project.dependencies.add("deviceCompileOnly", platform)
        project.dependencies.add(
            "deviceCompileOnly",
            "app.reseam:reseam-app-runtime:$reseamVersion",
        )

        val relocate =
            project.tasks.register("relocateKotlin", RelocateKotlinTask::class.java) {
                classes.from(device.output.classesDirs)
                kotlinPackage.set(project.kotlinPackage())
                output.set(project.layout.buildDirectory.dir("reseam/relocated"))
            }
        val dex =
            project.tasks.register("dex", DexTask::class.java) {
                group = "build"
                description = "Compiles the settings and strings the extensions read to DEX."
                sources.from(relocate.flatMap { it.output })
                libraries.from(platform)
                output.set(project.layout.buildDirectory.dir("reseam/dex"))
                globals.set(project.layout.buildDirectory.dir("reseam/globals"))
            }
        project.publish(ModuleVariant.SETTINGS, device)
        project.tasks.named("build") { dependsOn(dex) }
    }
}
