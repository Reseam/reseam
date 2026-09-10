// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.gradle

import org.gradle.api.JavaVersion
import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.api.plugins.JavaPluginExtension
import org.gradle.api.tasks.SourceSetContainer
import org.gradle.jvm.toolchain.JavaLanguageVersion

/**
 * A module of Java compiled against `android.jar` into one DEX file the
 * bundle ships. Sources under `src/stubs/java` are compile-time stand-ins for
 * classes the app already has; they are never dexed.
 */
class ReseamExtensionPlugin : Plugin<Project> {
    override fun apply(project: Project) {
        project.pluginManager.apply("java-library")
        project.extensions.configure(JavaPluginExtension::class.java) {
            toolchain.languageVersion.set(JavaLanguageVersion.of(17))
            sourceCompatibility = JavaVersion.VERSION_17
            targetCompatibility = JavaVersion.VERSION_17
        }
        val sourceSets = project.extensions.getByType(SourceSetContainer::class.java)
        val stubs = sourceSets.create("stubs")
        val main = sourceSets.getByName("main")
        main.compileClasspath += stubs.output
        val platform = project.files(project.provider { AndroidSdk.platformJar() })
        project.dependencies.add("compileOnly", platform)
        project.dependencies.add("stubsCompileOnly", platform)

        val dex = project.tasks.register("dex", DexTask::class.java) {
            group = "build"
            description = "Compiles the extension to DEX."
            sources.from(main.output.classesDirs)
            output.set(project.layout.buildDirectory.dir("reseam/dex"))
        }
        project.tasks.named("build") { dependsOn(dex) }
    }
}
