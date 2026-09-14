// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

plugins {
    kotlin("jvm")
    `maven-publish`
}

dependencies {
    implementation(kotlin("stdlib"))
}

kotlin {
    jvmToolchain(17)
    sourceSets.main { kotlin.srcDir("generated/app") }
    sourceSets.all { languageSettings.optIn("kotlin.ExperimentalUnsignedTypes") }
}

publishing {
    publications {
        create<MavenPublication>("maven") { from(components["java"]) }
    }
}
