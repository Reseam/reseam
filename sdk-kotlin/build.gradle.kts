// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

plugins {
    kotlin("multiplatform")
    id("com.android.kotlin.multiplatform.library")
    `maven-publish`
}

val rustSdk = rootProject.layout.projectDirectory.dir("sdk")
val desktopNatives = rustSdk.dir("dist/android/desktopJniLibs")

val androidNatives = layout.buildDirectory.dir("staged/jniLibs")
val stageJniLibs =
    tasks.register<Sync>("stageJniLibs") {
        from(rustSdk.dir("jniLibs")) { include("*/libreseam-sdk-native.so") }
        into(androidNatives)
    }

val desktopLibraries =
    listOf(
        "linux-x86_64/libreseam_sdk_native_jni.so",
        "windows-x86_64/reseam_sdk_native_jni.dll",
    )

val stageDesktopLibraries =
    tasks.register<Sync>("stageDesktopLibraries") {
        from(desktopNatives) { include(desktopLibraries) }
        into(layout.buildDirectory.dir("staged/desktop/native"))
        val required = desktopLibraries.map { desktopNatives.file(it).asFile }
        doFirst {
            required.forEach { library ->
                if (!library.isFile) {
                    throw GradleException(
                        "missing desktop SDK library: $library; run cargo xtask pack-sdk"
                    )
                }
            }
        }
    }

kotlin {
    jvmToolchain(17)
    android {
        namespace = "app.reseam.sdk"
        compileSdk = 36
        minSdk = 24
        optimization {
            consumerKeepRules.apply {
                publish = true
                file("consumer-rules.pro")
            }
        }
        compilerOptions { jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17) }
    }
    jvm()

    sourceSets {
        val jvmCommonMain =
            create("jvmCommonMain") {
                dependsOn(commonMain.get())
                kotlin.srcDir(rustSdk.dir("generated/app"))
                dependencies { api(project(":reseam-patch-sdk")) }
            }
        androidMain { dependsOn(jvmCommonMain) }
        jvmMain {
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

tasks
    .matching { it.name == "jvmProcessResources" }
    .configureEach { dependsOn(stageDesktopLibraries) }

tasks.matching { it.name.endsWith("JniLibFolders") }.configureEach { dependsOn(stageJniLibs) }

val patchApi = project(":reseam-patch-sdk")

evaluationDependsOn(patchApi.path)

val patchApiJar = patchApi.tasks.named<Jar>("jar")
val patchRuntime = patchApi.configurations.named("runtimeClasspath")

tasks.register<Jar>("patchRuntimeJar") {
    description = "Builds the patch API and Kotlin runtime embedded in native desktop hosts."
    archiveFileName.set("reseam-runtime.jar")
    destinationDirectory.set(rootProject.layout.buildDirectory.dir("runtime"))
    duplicatesStrategy = DuplicatesStrategy.EXCLUDE
    from(patchApiJar.map { zipTree(it.archiveFile) })
    from(patchRuntime.map { files -> files.filter { it.extension == "jar" }.map(::zipTree) })
    exclude(
        "META-INF/*.SF",
        "META-INF/*.RSA",
        "META-INF/*.DSA",
        "META-INF/versions/**",
        "module-info.class",
    )
}
