// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.gradle

import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.api.file.DuplicatesStrategy
import org.gradle.api.tasks.SourceSetContainer
import org.gradle.api.tasks.bundling.Jar
import org.jetbrains.kotlin.gradle.dsl.KotlinJvmProjectExtension

/**
 * A Kotlin module of patches. Builds a jar the engine loads on the desktop
 * JVM and on Android alike: the classes plus their `classes.dex`.
 */
class ReseamPatchesPlugin : Plugin<Project> {
    override fun apply(project: Project) {
        project.pluginManager.apply("org.jetbrains.kotlin.jvm")
        project.extensions.configure(KotlinJvmProjectExtension::class.java) { jvmToolchain(17) }
        project.dependencies.add("implementation", "app.reseam:reseam-patch-sdk:$reseamVersion")

        val reseam = project.extensions.create("reseam", ReseamPatchesExtension::class.java)
        val refs = project.tasks.register("generatePatchRefs", GeneratePatchRefsTask::class.java) {
            description = "Generates references to the patches of the bundles this module depends on."
            published.set(project.provider { reseam.published })
            local.from(project.provider { reseam.local })
            reseamBinary.set(project.reseamBinary())
            cache.set(project.layout.buildDirectory.dir("reseam/bundles"))
            output.set(project.layout.buildDirectory.dir("generated/reseam/refs"))
        }
        project.extensions.getByType(KotlinJvmProjectExtension::class.java).sourceSets.getByName("main").kotlin.srcDir(refs)

        val main = project.extensions.getByType(SourceSetContainer::class.java).getByName("main")
        val runtimeClasspath = project.configurations.getByName("runtimeClasspath")
        val classesJar = project.tasks.register("patchClassesJar", Jar::class.java) {
            description = "Bundles the patches with their dependencies."
            archiveFileName.set("${project.reseamArtifactName()}-classes.jar")
            destinationDirectory.set(project.layout.buildDirectory.dir("reseam"))
            duplicatesStrategy = DuplicatesStrategy.EXCLUDE
            from(main.output)
            from(project.provider { runtimeClasspath.filter { it.name.endsWith(".jar") }.map(project::zipTree) })
            exclude("META-INF/*.SF", "META-INF/*.DSA", "META-INF/*.RSA", "META-INF/*.kotlin_module")
        }
        val dex = project.tasks.register("patchDex", DexTask::class.java) {
            description = "Dexes the patch jar so it loads on Android."
            sources.from(classesJar)
            libraries.from(project.provider { AndroidSdk.platformJar() })
            output.set(project.layout.buildDirectory.dir("reseam/patch-dex"))
        }
        val universal = project.tasks.register("patchJar", Jar::class.java) {
            group = "build"
            description = "Builds the universal JVM/Android patch jar."
            archiveFileName.set("${project.reseamArtifactName()}-patches.jar")
            destinationDirectory.set(project.layout.buildDirectory.dir("reseam"))
            from(classesJar.map { project.zipTree(it.archiveFile) })
            from(dex.map { it.output }) { include("classes*.dex") }
        }
        project.tasks.named("build") { dependsOn(universal) }
    }
}
