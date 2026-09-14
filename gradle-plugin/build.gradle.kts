// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

plugins {
    kotlin("jvm")
    kotlin("plugin.sam.with.receiver")
    `java-gradle-plugin`
    `maven-publish`
}

samWithReceiver { annotation("org.gradle.api.HasImplicitReceiver") }

dependencies {
    implementation("org.jetbrains.kotlin:kotlin-gradle-plugin:2.4.10")
}

kotlin {
    jvmToolchain(17)
}

gradlePlugin {
    plugins {
        create("workspace") {
            id = "app.reseam.workspace"
            implementationClass = "app.reseam.gradle.ReseamWorkspacePlugin"
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
