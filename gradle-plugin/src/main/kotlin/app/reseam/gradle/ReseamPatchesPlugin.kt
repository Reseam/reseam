// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import java.util.zip.ZipFile
import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.api.file.DuplicatesStrategy
import org.gradle.api.tasks.ClasspathNormalizer
import org.gradle.api.tasks.SourceSetContainer
import org.gradle.api.tasks.bundling.Jar
import org.jetbrains.kotlin.gradle.dsl.KotlinJvmProjectExtension

internal class ReseamPatchesPlugin : Plugin<Project> {
    override fun apply(project: Project) {
        project.reseamArtifact()
        project.pluginManager.apply("org.jetbrains.kotlin.jvm")
        project.extensions.configure(KotlinJvmProjectExtension::class.java) { jvmToolchain(17) }
        project.dependencies.add("compileOnly", "app.reseam:reseam-patch-sdk:$reseamVersion")

        val reseam = project.extensions.create("reseam", ReseamPatchesExtension::class.java)
        val refs =
            project.tasks.register("generatePatchRefs", GeneratePatchRefsTask::class.java) {
                description =
                    "Generates references to the patches of the bundles this module depends on."
                published.set(project.provider { reseam.published })
                local.from(project.provider { reseam.local })
                reseamBinary.set(
                    project.layout.file(
                        project.provider {
                            if (reseam.published.isEmpty() && reseam.local.isEmpty()) null
                            else project.reseamExecutable().get()
                        }
                    )
                )
                cache.set(project.layout.buildDirectory.dir("reseam/bundles"))
                output.set(project.layout.buildDirectory.dir("generated/reseam/refs"))
            }
        project.extensions
            .getByType(KotlinJvmProjectExtension::class.java)
            .sourceSets
            .getByName("main")
            .kotlin
            .srcDir(refs)

        val main = project.extensions.getByType(SourceSetContainer::class.java).getByName("main")
        val dependencies =
            project.configurations
                .getByName("runtimeClasspath")
                .incoming
                .artifactView {
                    componentFilter { !isHostRuntime(it) }
                }
                .files
        val hostRuntime =
            project.configurations
                .getByName("compileClasspath")
                .incoming
                .artifactView {
                    componentFilter { isHostRuntime(it) }
                }
                .files
        val classesJar =
            project.tasks.register("patchClassesJar", Jar::class.java) {
                description = "Bundles the patches with their dependencies."
                archiveFileName.set(project.reseamArtifact().name.map { "$it-classes.jar" })
                destinationDirectory.set(project.layout.buildDirectory.dir("reseam"))
                duplicatesStrategy = DuplicatesStrategy.EXCLUDE
                inputs
                    .files(hostRuntime)
                    .withPropertyName("hostRuntime")
                    .withNormalizer(ClasspathNormalizer::class.java)
                from(main.output)
                from(
                    project.provider {
                        dependencies.filter { it.name.endsWith(".jar") }.map(project::zipTree)
                    }
                )
                exclude(
                    PATCH_INDEX,
                    "META-INF/*.SF",
                    "META-INF/*.DSA",
                    "META-INF/*.RSA",
                    "META-INF/*.kotlin_module",
                )
                // Dependencies may themselves be fat jars. Match actual host classes
                // rather than packages: kotlin-reflect also lives under kotlin/.
                doFirst {
                    val provided =
                        hostRuntime.files.flatMap { library ->
                            if (library.isDirectory) {
                                library
                                    .walkTopDown()
                                    .onFail { file, error ->
                                        throw org.gradle.api.GradleException(
                                            "cannot read host runtime classes in $file",
                                            error,
                                        )
                                    }
                                    .filter { it.isFile && it.extension == "class" }
                                    .map { it.relativeTo(library).invariantSeparatorsPath }
                                    .toList()
                            } else {
                                ZipFile(library).use { jar ->
                                    jar.entries()
                                        .asSequence()
                                        .filter { !it.isDirectory && it.name.endsWith(".class") }
                                        .map { it.name }
                                        .toList()
                                }
                            }
                        }
                    exclude(provided)
                }
            }
        val dex =
            project.tasks.register("patchDex", DexTask::class.java) {
                description = "Dexes the patch jar so it loads on Android."
                sources.from(classesJar)
                libraries.from(hostRuntime, project.provider { AndroidSdk.platformJar() })
                output.set(project.layout.buildDirectory.dir("reseam/patch-dex"))
            }
        val index =
            project.tasks.register("patchIndex", PatchIndexTask::class.java) {
                jar.set(classesJar.flatMap { it.archiveFile })
                libraries.from(main.compileClasspath)
                output.set(project.layout.buildDirectory.file("reseam/index/patches.json"))
            }
        val universal =
            project.tasks.register("patchJar", Jar::class.java) {
                group = "build"
                description = "Builds the universal JVM/Android patch jar."
                archiveFileName.set(project.reseamArtifact().name.map { "$it-patches.jar" })
                destinationDirectory.set(project.layout.buildDirectory.dir("reseam"))
                from(classesJar.map { project.zipTree(it.archiveFile) })
                from(dex.map { it.output }) { include("classes*.dex") }
                from(index.flatMap { it.output }) { into("META-INF/reseam") }
            }
        project.tasks.named("build") { dependsOn(universal) }
    }
}

private fun isHostRuntime(id: org.gradle.api.artifacts.component.ComponentIdentifier): Boolean =
    when (id) {
        is org.gradle.api.artifacts.component.ModuleComponentIdentifier ->
            (id.group == "app.reseam" && id.module == "reseam-patch-sdk") ||
                (id.group == "org.jetbrains.kotlin" && id.module.startsWith("kotlin-stdlib"))
        is org.gradle.api.artifacts.component.ProjectComponentIdentifier ->
            id.projectName == "reseam-patch-sdk"
        else -> false
    }
