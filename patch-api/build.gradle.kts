// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

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
}

tasks.test {
    val libDir = rootProject.layout.projectDirectory.dir("target/debug").asFile.path
    jvmArgs("-Djava.library.path=$libDir")
    environment("LD_LIBRARY_PATH", libDir)
}

publishing {
    publications {
        create<MavenPublication>("maven") { from(components["java"]) }
    }
}
