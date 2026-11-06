// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import org.gradle.api.Project
import org.gradle.api.file.FileCollection
import org.gradle.api.provider.Property
import org.gradle.api.provider.Provider
import org.gradle.api.tasks.TaskProvider
import org.jetbrains.kotlin.gradle.dsl.KotlinJvmProjectExtension
import org.jetbrains.kotlin.gradle.plugin.KotlinSourceSet
import org.jetbrains.kotlin.gradle.plugin.getKotlinPluginVersion
import org.jetbrains.kotlin.gradle.tasks.KotlinCompile

internal fun Project.kotlinStdlib(): FileCollection =
    configurations
        .detachedConfiguration(
            dependencies.create("org.jetbrains.kotlin:kotlin-stdlib:${getKotlinPluginVersion()}")
        )
        .apply { isTransitive = false }

/** The stdlib reaches the app through the relocated, shrunk copy only, never as a dependency. */
internal fun Project.applyExtensionKotlin() {
    pluginManager.apply("org.jetbrains.kotlin.jvm")
    configurations.named("runtimeClasspath") {
        exclude(mapOf("group" to "org.jetbrains.kotlin"))
        exclude(mapOf("group" to "org.jetbrains", "module" to "annotations"))
    }
    extensions.configure(KotlinJvmProjectExtension::class.java) {
        jvmToolchain(17)
    }
    tasks.withType(KotlinCompile::class.java).configureEach {
        compilerOptions.freeCompilerArgs.addAll(
            "-Xno-param-assertions",
            "-Xno-call-assertions",
            "-Xno-receiver-assertions",
        )
    }
}

/** The bundle a workspace builds; its extensions share one relocated stdlib. */
internal abstract class ReseamBundleInfo {
    abstract val name: Property<String>

    val kotlinPackage: Provider<String>
        get() = name.map { "app.reseam.kotlin." + it.lowercase().replace(Regex("[^a-z0-9]"), "_") }
}

internal fun Project.inBundle(): Boolean =
    rootProject.plugins.hasPlugin(ReseamBundlePlugin::class.java)

internal fun Project.bundleInfo(): ReseamBundleInfo =
    rootProject.extensions.getByType(ReseamBundleInfo::class.java)

internal fun Project.kotlinPackage(): Provider<String> =
    if (inBundle()) bundleInfo().kotlinPackage else reseamArtifact().kotlinPackage

/** The standard library relocated to [kotlinPackage], and the part of it that [roots] use. */
internal class KotlinStdlib(
    val relocated: TaskProvider<RelocateKotlinTask>,
    val shrunk: TaskProvider<KotlinStdlibTask>,
)

internal fun Project.registerKotlinStdlib(roots: Any, platform: FileCollection): KotlinStdlib {
    val relocated =
        tasks.register("relocateKotlinStdlib", RelocateKotlinTask::class.java) {
            classes.from(kotlinStdlib())
            kotlinPackage.set(project.kotlinPackage())
            output.set(layout.buildDirectory.dir("reseam/kotlin-stdlib-relocated"))
        }
    val shrunk =
        tasks.register("kotlinStdlib", KotlinStdlibTask::class.java) {
            description = "Shrinks the Kotlin stdlib to what the roots use."
            this.roots.from(roots)
            stdlib.from(relocated.flatMap { it.output })
            libraries.from(platform)
            kotlinPackage.set(project.kotlinPackage())
            output.set(layout.buildDirectory.dir("reseam/kotlin-stdlib"))
        }
    return KotlinStdlib(relocated, shrunk)
}

internal fun Project.kotlinSourceSet(name: String): KotlinSourceSet =
    extensions.getByType(KotlinJvmProjectExtension::class.java).sourceSets.getByName(name)
