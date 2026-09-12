// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.gradle

import org.gradle.api.JavaVersion
import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.api.attributes.LibraryElements
import org.gradle.api.plugins.JavaPluginExtension
import org.gradle.api.tasks.SourceSetContainer
import org.gradle.jvm.toolchain.JavaLanguageVersion

/**
 * A module of Java compiled against `android.jar` into one DEX file the
 * bundle ships. `implementation` dependencies are dexed into it; `compileOnly`
 * ones are expected from the app or another module of the bundle. Sources
 * under `src/stubs/java` are compile-time stand-ins for classes the app
 * already has; they are never dexed.
 */
class ReseamExtensionPlugin : Plugin<Project> {
    override fun apply(project: Project) {
        project.pluginManager.apply("java-library")
        project.extensions.configure(JavaPluginExtension::class.java) {
            toolchain.languageVersion.set(JavaLanguageVersion.of(17))
            sourceCompatibility = JavaVersion.VERSION_17
            targetCompatibility = JavaVersion.VERSION_17
        }
        project.dependencies.attributesSchema.attribute(LibraryElements.LIBRARY_ELEMENTS_ATTRIBUTE).compatibilityRules.add(AarElementsCompatibility::class.java)
        project.dependencies.registerTransform(AarClassesTransform::class.java) {
            from.attribute(ARTIFACT_TYPE, "aar")
            to.attribute(ARTIFACT_TYPE, "jar")
        }
        listOf("compileClasspath", "runtimeClasspath").forEach { project.configurations.getByName(it).attributes.attribute(ARTIFACT_TYPE, "jar") }
        val sourceSets = project.extensions.getByType(SourceSetContainer::class.java)
        val stubs = sourceSets.create("stubs")
        val main = sourceSets.getByName("main")
        main.compileClasspath += stubs.output
        val platform = project.files(project.provider { AndroidSdk.platformJar() })
        project.dependencies.add("compileOnly", platform)
        project.dependencies.add("stubsCompileOnly", platform)

        val dex = project.tasks.register("dex", DexTask::class.java) {
            group = "build"
            description = "Compiles the extension and its dependencies to DEX."
            sources.from(main.output.classesDirs, project.configurations.getByName("runtimeClasspath"))
            libraries.from(platform)
            output.set(project.layout.buildDirectory.dir("reseam/dex"))
        }
        project.tasks.named("build") { dependsOn(dex) }
    }
}
