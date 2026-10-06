// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

plugins {
    kotlin("jvm")
    kotlin("plugin.sam.with.receiver")
    alias(libs.plugins.kotlin.serialization)
    `java-gradle-plugin`
    `maven-publish`
}

samWithReceiver { annotation("org.gradle.api.HasImplicitReceiver") }

dependencies {
    implementation(libs.kotlin.gradle.plugin)
    implementation(libs.kotlinx.serialization.json)
    implementation("org.ow2.asm:asm:9.10")
}

kotlin {
    jvmToolchain(17)
}

val indexBindings =
    tasks.register<JavaExec>("generatePatchIndexBindings") {
        description = "Checks the engine's bindings against the patch index serialization model."
        classpath = sourceSets.main.get().runtimeClasspath
        mainClass.set("app.reseam.gradle.PatchIndexFormatKt")
        val bindings = rootProject.file("crates/patcher/src/bundle/index.rs")
        inputs.file(bindings)
        args(bindings.absolutePath)
        systemProperty(
            "reseam.updateIndexBindings",
            providers.systemProperty("reseam.updateIndexBindings").getOrElse("false"),
        )
    }

tasks.check { dependsOn(indexBindings) }

tasks.jar { dependsOn(indexBindings) }

gradlePlugin {
    plugins {
        create("workspace") {
            id = "app.reseam.workspace"
            implementationClass = "app.reseam.gradle.ReseamWorkspacePlugin"
        }
        create("bundle") {
            id = "app.reseam.bundle"
            implementationClass = "app.reseam.gradle.ReseamBundlePlugin"
        }
        create("patches") {
            id = "app.reseam.patches"
            implementationClass = "app.reseam.gradle.ReseamPatchesPlugin"
        }
        create("extension") {
            id = "app.reseam.extension"
            implementationClass = "app.reseam.gradle.ReseamExtensionPlugin"
        }
    }
}

tasks.processResources {
    val version = project.version.toString()
    inputs.property("version", version)
    filesMatching("META-INF/reseam.properties") {
        expand("version" to version)
    }
}
