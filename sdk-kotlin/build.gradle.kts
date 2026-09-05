// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

import java.util.Locale

plugins {
    kotlin("multiplatform")
    id("com.android.library")
    `maven-publish`
}

val rustSdk = rootProject.layout.projectDirectory.dir("sdk")
val rustRelease = rootProject.layout.projectDirectory.dir("target/release")
val androidAbis = listOf("arm64-v8a", "armeabi-v7a", "x86", "x86_64")

val desktopHost: String = run {
    val os = System.getProperty("os.name").lowercase(Locale.ROOT)
    val arch = System.getProperty("os.arch").lowercase(Locale.ROOT)
    val family = when {
        os.contains("linux") -> "linux"
        os.contains("mac") || os.contains("darwin") -> "darwin"
        else -> throw GradleException("unsupported desktop host: $os")
    }
    val cpu = when (arch) {
        "amd64", "x86_64" -> "x86_64"
        "aarch64", "arm64" -> if (family == "darwin") "arm64" else "aarch64"
        else -> throw GradleException("unsupported desktop arch: $arch")
    }
    "$family-$cpu"
}
val desktopShim = if (desktopHost.startsWith("darwin-")) "libreseam_sdk_jni.dylib" else "libreseam_sdk_jni.so"

val stageJniLibs by tasks.registering(Sync::class) {
    androidAbis.forEach { abi ->
        from(rustSdk.file("jniLibs/$abi/libreseam-sdk.so")) {
            into(abi)
            rename { "libreseam_sdk.so" }
        }
    }
    into(layout.buildDirectory.dir("staged/jniLibs"))
}

val stageDesktopShim by tasks.registering(Sync::class) {
    from(rustRelease.file(desktopShim))
    into(layout.buildDirectory.dir("staged/desktop/native/$desktopHost"))
}

kotlin {
    jvmToolchain(17)
    androidTarget { publishLibraryVariants("release") }
    jvm()

    sourceSets {
        val commonMain by getting
        val jvmCommonMain by creating {
            dependsOn(commonMain)
            kotlin.srcDir(rustSdk.dir("generated/app"))
        }
        val androidMain by getting { dependsOn(jvmCommonMain) }
        val jvmMain by getting {
            dependsOn(jvmCommonMain)
            resources.srcDir(layout.buildDirectory.dir("staged/desktop"))
        }
    }
}

android {
    namespace = "app.reseam.sdk"
    compileSdk = 36
    defaultConfig { minSdk = 24 }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    sourceSets["main"].jniLibs.srcDir(layout.buildDirectory.dir("staged/jniLibs"))
}

tasks.matching { it.name == "jvmProcessResources" }.configureEach { dependsOn(stageDesktopShim) }
tasks.matching { it.name.startsWith("merge") && it.name.endsWith("JniLibFolders") }.configureEach { dependsOn(stageJniLibs) }
