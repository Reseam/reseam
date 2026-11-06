// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

plugins {
    kotlin("jvm")
    `maven-publish`
}

dependencies {
    implementation(kotlin("stdlib"))
    testImplementation(kotlin("test"))
}

kotlin {
    jvmToolchain(17)
    @OptIn(org.jetbrains.kotlin.gradle.dsl.abi.ExperimentalAbiValidation::class)
    abiValidation {
        filters { exclude { byNames.add("app.reseam.patch.native.**") } }
    }
    sourceSets.main { kotlin.srcDir("generated/app") }
    sourceSets.all { languageSettings.optIn("kotlin.ExperimentalUnsignedTypes") }
}

// Settings hosts write the app runtime's string table into every patched app.
tasks.test { useJUnitPlatform() }

tasks.processResources {
    dependsOn(gradle.includedBuild("app-runtime").task(":stage"))
    from(rootDir.resolve("build/runtime/app/strings")) { into("reseam/strings/runtime") }
}

publishing {
    publications {
        create<MavenPublication>("maven") { from(components["java"]) }
    }
}
