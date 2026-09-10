// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

plugins {
    `kotlin-dsl`
    `maven-publish`
}

dependencies {
    implementation("org.jetbrains.kotlin:kotlin-gradle-plugin:1.9.25")
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
    }
}

tasks.processResources {
    val version = project.version.toString()
    inputs.property("version", version)
    filesMatching("META-INF/reseam.properties") {
        expand("version" to version)
    }
}
