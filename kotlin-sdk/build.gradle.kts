// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

plugins {
    kotlin("jvm") version "1.9.25"
    `maven-publish`
}

group = "app.reseam"
version = "0.1.0"

// `generated/` is the raw BoltFFI staging area. The publishable Kotlin sources live in `src/main`.

repositories {
    mavenCentral()
}

dependencies {
    implementation(kotlin("stdlib"))
    testImplementation(kotlin("test"))
}

tasks.test {
    val libDir = "${rootProject.projectDir}/../target/debug"
    jvmArgs("-Djava.library.path=$libDir")
    environment("LD_LIBRARY_PATH", libDir)
}

tasks.register<Exec>("syncPublishedBridge") {
    workingDir = projectDir
    commandLine("./fix-generated.sh")
}

kotlin {
    jvmToolchain(17)
}

publishing {
    publications {
        create<MavenPublication>("maven") {
            groupId = "app.reseam"
            artifactId = "reseam-patch-sdk"
            version = project.version.toString()
            from(components["java"])
        }
    }
}
