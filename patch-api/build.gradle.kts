// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

import org.gradle.api.credentials.HttpHeaderCredentials
import org.gradle.authentication.http.HttpHeaderAuthentication

plugins {
    kotlin("jvm") version "1.9.25"
    `maven-publish`
}

group = "app.reseam"
version = providers.gradleProperty("reseamSdkVersion").orElse("0.1.0").get()

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

    repositories {
        maven {
            name = "Forgejo"
            url = uri("https://git.reseam.app/api/packages/reseam/maven")

            credentials(HttpHeaderCredentials::class) {
                name = "Authorization"
                value = providers.environmentVariable("FORGEJO_PACKAGES_TOKEN")
                    .map { "token $it" }
                    .orElse("")
                    .get()
            }

            authentication {
                create<HttpHeaderAuthentication>("header")
            }
        }
    }
}
