// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.gradle

import org.gradle.api.JavaVersion
import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.api.attributes.LibraryElements
import org.gradle.api.attributes.Usage
import org.gradle.api.plugins.JavaPluginExtension
import org.gradle.api.tasks.SourceSetContainer
import org.gradle.jvm.toolchain.JavaLanguageVersion

internal const val NATIVE_MODULE_TYPES = "nativeModuleTypes"

private val JAVA_USAGES = setOf(Usage.JAVA_API, Usage.JAVA_RUNTIME)

internal class ReseamExtensionPlugin : Plugin<Project> {
    override fun apply(project: Project) {
        if (project.file("src/main/js").isDirectory) {
            project.pluginManager.apply(ReseamJavascriptPlugin::class.java)
            return
        }
        project.reseamArtifact()
        project.pluginManager.apply("java-library")
        project.applyExtensionKotlin()
        project.extensions.configure(JavaPluginExtension::class.java) {
            toolchain.languageVersion.set(JavaLanguageVersion.of(17))
            sourceCompatibility = JavaVersion.VERSION_17
            targetCompatibility = JavaVersion.VERSION_17
        }
        project.dependencies.attributesSchema
            .attribute(LibraryElements.LIBRARY_ELEMENTS_ATTRIBUTE)
            .compatibilityRules
            .add(AarElementsCompatibility::class.java)
        project.dependencies.registerTransform(AarClassesTransform::class.java) {
            from.attribute(ARTIFACT_TYPE, "aar")
            to.attribute(ARTIFACT_TYPE, "jar")
        }
        // Every classpath, including ones other plugins add (protobuf's compileProtoPath), reads
        // Android libraries through the transform.
        project.afterEvaluate {
            configurations
                .filter {
                    it.isCanBeResolved &&
                        it.attributes.getAttribute(Usage.USAGE_ATTRIBUTE)?.name in JAVA_USAGES
                }
                .forEach {
                    it.attributes {
                        attribute(ARTIFACT_TYPE, "jar")
                        attribute(BUILD_TYPE, "release")
                    }
                }
        }
        val runtime = project.configurations.getByName("runtimeClasspath")
        val sourceSets = project.extensions.getByType(SourceSetContainer::class.java)
        val stubs = sourceSets.create("stubs")
        val main = sourceSets.getByName("main")
        val settings = project.resolve(ModuleVariant.SETTINGS)
        main.compileClasspath += stubs.output + settings
        val platform = project.files(project.provider { AndroidSdk.platformJar() })
        project.dependencies.add("compileOnly", platform)
        project.dependencies.add("stubsCompileOnly", platform)
        project.configureExtensionTests(sourceSets.getByName("test"), stubs.output, settings)

        project.reseamStrings()?.let { strings ->
            project.registerStringTables(strings)
            project
                .kotlinSourceSet("main")
                .kotlin
                .srcDir(project.registerDeviceStrings(strings).flatMap { it.device })
        }
        fun relocate(name: String, classes: Any, into: String) =
            project.tasks.register(name, RelocateKotlinTask::class.java) {
                this.classes.from(classes)
                appClasses.from(stubs.output.classesDirs)
                kotlinPackage.set(project.kotlinPackage())
                output.set(project.layout.buildDirectory.dir(into))
            }
        val relocate = relocate("relocateKotlin", main.output.classesDirs, "reseam/relocated")
        val relocateDependencies =
            relocate("relocateDependencies", runtime, "reseam/dependencies-relocated")
        val aars =
            runtime.incoming
                .artifactView {
                    // Plain jars have no aar variant; real resolution failures still fail the
                    // runtime classpath.
                    lenient(true)
                    attributes.attribute(ARTIFACT_TYPE, "aar")
                }
                .files
        val dependencies =
            project.tasks.register("shrinkDependencies", ShrinkTask::class.java) {
                description = "Shrinks the extension's dependencies to what it uses."
                roots.from(relocate.flatMap { it.output })
                traced.from(relocateDependencies.flatMap { it.output })
                ruleSources.from(runtime, aars)
                libraries.from(
                    platform,
                    stubs.output.classesDirs,
                    project.relocatedKotlinStdlib(),
                )
                output.set(project.layout.buildDirectory.dir("reseam/dependencies"))
            }
        // Outside a bundle the extension ships its own stdlib, and callers relocate theirs
        // elsewhere.
        val standalone =
            if (project.inBundle()) null
            else
                project
                    .registerKotlinStdlib(
                        project.files(
                            relocate.flatMap { it.output },
                            dependencies.flatMap { it.output },
                        ),
                        platform,
                    )
                    .shrunk
        val apiCheck =
            project.tasks.register("checkKotlinFreeApi", KotlinFreeApiTask::class.java) {
                classes.from(main.output.classesDirs)
            }
        val dex =
            project.tasks.register("dex", DexTask::class.java) {
                group = "build"
                description = "Compiles the extension and its dependencies to DEX."
                sources.from(relocate.flatMap { it.output }, dependencies.flatMap { it.output })
                shippedAars.from(aars)
                libraries.from(platform)
                output.set(project.layout.buildDirectory.dir("reseam/dex"))
                if (standalone == null) {
                    globals.set(project.layout.buildDirectory.dir("reseam/globals"))
                } else {
                    sources.from(standalone.flatMap { it.output })
                    dependsOn(apiCheck)
                }
            }
        project.tasks.register(NATIVE_MODULE_TYPES, NativeModuleTypesTask::class.java) {
            group = "build"
            description = "Declares the extension's React Native modules for TypeScript."
            classes.from(main.output.classesDirs)
            output.set(project.layout.buildDirectory.dir("reseam/types"))
        }
        project.publish(ModuleVariant.EXPORTS, main)
        project.tasks.named("build") { dependsOn(dex) }
    }
}
