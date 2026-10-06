// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

plugins {
    kotlin("jvm")
    `maven-publish`
}

dependencies {
    implementation(kotlin("stdlib"))
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

publishing {
    publications {
        create<MavenPublication>("maven") { from(components["java"]) }
    }
}
