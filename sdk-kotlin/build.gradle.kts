// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

import java.util.Locale

plugins {
    kotlin("multiplatform")
    id("com.android.kotlin.multiplatform.library")
    `maven-publish`
}

val rustSdk = rootProject.layout.projectDirectory.dir("sdk")
val desktopNatives = rustSdk.dir("dist/android/desktopJniLibs")

val androidNatives = layout.buildDirectory.dir("staged/jniLibs")
val stageJniLibs by tasks.registering(Sync::class) {
    from(rustSdk.dir("jniLibs")) { include("*/libreseam-sdk-native.so") }
    into(androidNatives)
}

val desktopHost: String = run {
    val os = System.getProperty("os.name").lowercase(Locale.ROOT)
    val arch = System.getProperty("os.arch").lowercase(Locale.ROOT)
    val family = when {
        os.contains("linux") -> "linux"
        os.contains("mac") || os.contains("darwin") -> "darwin"
        os.contains("windows") -> "windows"
        else -> throw GradleException("unsupported desktop host: $os")
    }
    val cpu = when (arch) {
        "amd64", "x86_64" -> "x86_64"
        "aarch64", "arm64" -> if (family == "darwin") "arm64" else "aarch64"
        else -> throw GradleException("unsupported desktop arch: $arch")
    }
    "$family-$cpu"
}
val desktopShim = System.mapLibraryName("reseam_sdk_native_jni")

val stageDesktopShim by tasks.registering(Sync::class) {
    from(desktopNatives.file("$desktopHost/$desktopShim"))
    into(layout.buildDirectory.dir("staged/desktop/native/$desktopHost"))
}

kotlin {
    jvmToolchain(17)
    android {
        namespace = "app.reseam.sdk"
        compileSdk = 36
        minSdk = 24
        compilerOptions { jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17) }
    }
    jvm()

    sourceSets {
        val commonMain by getting
        val jvmCommonMain by creating {
            dependsOn(commonMain)
            kotlin.srcDir(rustSdk.dir("generated/app"))
            dependencies { api(project(":reseam-patch-sdk")) }
        }
        val androidMain by getting { dependsOn(jvmCommonMain) }
        val jvmMain by getting {
            dependsOn(jvmCommonMain)
            resources.srcDir(layout.buildDirectory.dir("staged/desktop"))
        }
    }
}

androidComponents {
    onVariants { variant ->
        variant.sources.jniLibs?.addStaticSourceDirectory(androidNatives.get().asFile.path)
    }
}

tasks.matching { it.name == "jvmProcessResources" }.configureEach { dependsOn(stageDesktopShim) }

tasks.matching { it.name.endsWith("JniLibFolders") }.configureEach { dependsOn(stageJniLibs) }
