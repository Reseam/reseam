// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

import org.gradle.api.credentials.HttpHeaderCredentials
import org.gradle.authentication.http.HttpHeaderAuthentication
import java.io.File
import java.util.Locale

abstract class StageNativeShim : DefaultTask() {
    @get:Input
    abstract val sourcePath: Property<String>

    @get:Input
    abstract val classifier: Property<String>

    @get:Input
    abstract val targetFileName: Property<String>

    @get:OutputDirectory
    abstract val outputDir: DirectoryProperty

    @TaskAction
    fun stage() {
        val src = File(sourcePath.get())
        check(src.isFile) {
            "Desktop JNI shim missing at $src. Run `JAVA_HOME=... cargo xtask jni-host --crate sdk` first."
        }
        val out = outputDir.get().asFile
        out.deleteRecursively()
        val dest = out.resolve("native/${classifier.get()}/${targetFileName.get()}")
        dest.parentFile.mkdirs()
        src.copyTo(dest, overwrite = true)
    }
}

plugins {
    kotlin("jvm") version "1.9.25"
    `maven-publish`
}

group = "app.reseam"
version = providers.gradleProperty("reseamSdkVersion").orElse("0.1.0").get()

repositories { mavenCentral() }

val workspaceRoot = projectDir.parentFile
val sdkGenerated = workspaceRoot.resolve("sdk/generated/app")
val rustReleaseDir = workspaceRoot.resolve("target/release")

fun detectHostClassifier(): String {
    val osName = System.getProperty("os.name").lowercase(Locale.ROOT)
    val osArch = System.getProperty("os.arch").lowercase(Locale.ROOT)
    return when {
        osName.contains("linux") && (osArch == "amd64" || osArch == "x86_64") -> "linux-x86_64"
        osName.contains("linux") && (osArch == "aarch64" || osArch == "arm64") -> "linux-aarch64"
        (osName.contains("mac") || osName.contains("darwin")) &&
            (osArch == "aarch64" || osArch == "arm64") -> "darwin-arm64"
        (osName.contains("mac") || osName.contains("darwin")) && osArch == "x86_64" -> "darwin-x86_64"
        else -> throw GradleException("unsupported host: $osName $osArch")
    }
}

val hostClassifier = providers.gradleProperty("reseamHostClassifier")
    .orElse(provider { detectHostClassifier() })
    .get()

val hostNativeFile = when {
    hostClassifier.startsWith("darwin-") -> "libreseam_sdk_jni.dylib"
    else -> "libreseam_sdk_jni.so"
}

val stagedNativeDir = layout.buildDirectory.dir("staged-native")

val stageNativeShim = tasks.register<StageNativeShim>("stageNativeShim") {
    sourcePath.set(rustReleaseDir.resolve(hostNativeFile).absolutePath)
    classifier.set(hostClassifier)
    targetFileName.set(hostNativeFile)
    outputDir.set(stagedNativeDir)
}

kotlin {
    jvmToolchain(17)
}

sourceSets {
    main {
        java.srcDir(sdkGenerated)
        java.exclude("**/ReseamAndroidHost.kt")
        resources.srcDir(stagedNativeDir)
    }
}

tasks.named<ProcessResources>("processResources") {
    dependsOn(stageNativeShim)
}

tasks.jar {
    archiveBaseName.set("reseam-sdk-jvm")
    archiveClassifier.set(hostClassifier)
}

publishing {
    publications {
        create<MavenPublication>("maven") {
            from(components["java"])
            groupId = "app.reseam"
            artifactId = "reseam-sdk-jvm"
            version = project.version.toString()
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
