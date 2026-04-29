// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

import org.gradle.api.credentials.HttpHeaderCredentials
import org.gradle.authentication.http.HttpHeaderAuthentication
import java.io.File

data class AbiSpec(
    @get:Input val abi: String,
    @get:Input val sdkLibPath: String,
    @get:Input val patcherLibPath: String,
) : java.io.Serializable

abstract class StageJniLibs : DefaultTask() {
    @get:Nested
    abstract val abiSpecs: ListProperty<AbiSpec>

    @get:OutputDirectory
    abstract val outputDir: DirectoryProperty

    @TaskAction
    fun stage() {
        val out = outputDir.get().asFile
        out.deleteRecursively()
        out.mkdirs()
        for (spec in abiSpecs.get()) {
            val sdkSo = File(spec.sdkLibPath)
            val patcherSo = File(spec.patcherLibPath)
            check(sdkSo.isFile) {
                "Missing $sdkSo. Run `cargo xtask regen sdk` first."
            }
            check(patcherSo.isFile) {
                "Missing $patcherSo. Run `cargo xtask regen sdk` first."
            }
            val abiOut = out.resolve(spec.abi).also { it.mkdirs() }
            sdkSo.copyTo(abiOut.resolve("libreseam-sdk.so"), overwrite = true)
            patcherSo.copyTo(abiOut.resolve("libreseam_patcher.so"), overwrite = true)
        }
    }
}

plugins {
    id("com.android.library") version "8.11.2"
    kotlin("android") version "1.9.25"
    `maven-publish`
}

group = "app.reseam"
version = providers.gradleProperty("reseamSdkVersion").orElse("0.1.0").get()

val workspaceRoot = projectDir.parentFile
val sdkGenerated = workspaceRoot.resolve("sdk/generated/app")
val sdkJniLibs = workspaceRoot.resolve("sdk/jniLibs")
val rustTargetDir = workspaceRoot.resolve("target")

val androidAbis = listOf(
    "arm64-v8a" to "aarch64-linux-android",
    "armeabi-v7a" to "armv7-linux-androideabi",
    "x86" to "i686-linux-android",
    "x86_64" to "x86_64-linux-android",
)

val stagedJniLibs = layout.buildDirectory.dir("staged-jni-libs")

val stageJniLibs = tasks.register<StageJniLibs>("stageJniLibs") {
    outputDir.set(stagedJniLibs)
    abiSpecs.set(androidAbis.map { (abi, triple) ->
        AbiSpec(
            abi = abi,
            sdkLibPath = sdkJniLibs.resolve("$abi/libreseam-sdk.so").absolutePath,
            patcherLibPath = rustTargetDir.resolve("$triple/release/deps/libreseam_patcher.so").absolutePath,
        )
    })
}

android {
    namespace = "app.reseam.sdk"
    compileSdk = 36

    defaultConfig {
        minSdk = 24
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    sourceSets["main"].kotlin.srcDir(sdkGenerated)
    sourceSets["main"].jniLibs.srcDir(stagedJniLibs)

    publishing {
        singleVariant("release") {
            withSourcesJar()
        }
    }
}

kotlin {
    jvmToolchain(17)
}

tasks.named("preBuild") { dependsOn(stageJniLibs) }

afterEvaluate {
    publishing {
        publications {
            create<MavenPublication>("maven") {
                from(components["release"])
                groupId = "app.reseam"
                artifactId = "reseam-sdk-android"
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
}
