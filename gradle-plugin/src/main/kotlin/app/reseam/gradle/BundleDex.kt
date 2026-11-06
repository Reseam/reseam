// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import org.gradle.api.Project
import org.gradle.api.provider.Provider
import org.gradle.api.tasks.TaskProvider

/**
 * The bundle's DEX: one per extension and settings module, the Kotlin stdlib shrunk to what they
 * use, and d8's globals.
 */
internal class BundleDex(
    val modules: Provider<List<TaskProvider<DexTask>>>,
    val kotlinStdlib: KotlinStdlib,
    val kotlin: TaskProvider<DexTask>,
    val globals: TaskProvider<GlobalSyntheticsDexTask>,
)

internal fun Project.registerBundleDex(): BundleDex {
    val deviceProjects = provider {
        subprojects.filter {
            (it.plugins.hasPlugin(ReseamExtensionPlugin::class.java) &&
                !it.plugins.hasPlugin(ReseamJavascriptPlugin::class.java)) ||
                it.plugins.hasPlugin(ReseamSettingsPlugin::class.java)
        }
    }
    val platform = files(provider { AndroidSdk.platformJar() })
    val kotlinStdlib =
        registerKotlinStdlib(
            deviceProjects.map { projects ->
                projects.map { p ->
                    p.tasks.named("relocateKotlin", RelocateKotlinTask::class.java).flatMap {
                        it.output
                    }
                }
            },
            platform,
        )
    val kotlinDex =
        tasks.register("dexKotlinStdlib", DexTask::class.java) {
            sources.from(kotlinStdlib.shrunk.flatMap { it.output })
            allowEmpty.set(true)
            libraries.from(platform)
            output.set(layout.buildDirectory.dir("reseam/kotlin-dex"))
            globals.set(layout.buildDirectory.dir("reseam/kotlin-globals"))
        }
    val extensionDex = deviceProjects.map { projects ->
        projects.map { it.tasks.named("dex", DexTask::class.java) }
    }
    val globalsDex =
        tasks.register("dexGlobalSynthetics", GlobalSyntheticsDexTask::class.java) {
            description = "Compiles the extensions' d8 global synthetics into one shared DEX."
            (extensionDex.get() + kotlinDex).forEach { dexTask ->
                globals.from(dexTask.flatMap { it.globals }.map { it.file("classes.globals") })
                dependsOn(dexTask)
            }
            libraries.from(platform)
            output.set(layout.buildDirectory.dir("reseam/globals-dex"))
        }

    return BundleDex(extensionDex, kotlinStdlib, kotlinDex, globalsDex)
}
