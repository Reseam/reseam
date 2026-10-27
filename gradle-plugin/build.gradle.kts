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
    implementation("org.ow2.asm:asm-commons:9.10")
    implementation("org.jetbrains.kotlin:kotlin-metadata-jvm:${libs.versions.kotlin.get()}")
    testImplementation(kotlin("test"))
}

kotlin {
    jvmToolchain(17)
}

tasks.test {
    useJUnitPlatform()
    systemProperty(
        "reseam.patchIndexFixture",
        rootProject.file("crates/patcher/tests/fixtures/patch-index.json").absolutePath,
    )
}

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
        create("settings") {
            id = "app.reseam.settings"
            implementationClass = "app.reseam.gradle.ReseamSettingsPlugin"
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
