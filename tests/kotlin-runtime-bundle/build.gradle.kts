// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

plugins {
    id("app.reseam.patches")
}

repositories {
    google { mavenContent { includeGroupAndSubgroups("com.android") } }
    mavenCentral()
}

(extensions.findByType(app.reseam.gradle.ReseamArtifactExtension::class.java)
        ?: extensions.create(
            "reseamArtifact",
            app.reseam.gradle.ReseamArtifactExtension::class.java,
        ))
    .name
    .set("reseam-test")

// This fixture exercises the internal JNI bridge as well as the authoring API.
tasks.withType<org.jetbrains.kotlin.gradle.tasks.KotlinCompile>().configureEach {
    compilerOptions.freeCompilerArgs.add(
        libraries.elements.map { classpath ->
            val sdk = classpath.single { it.asFile.name.startsWith("reseam-patch-sdk-") }
            "-Xfriend-paths=${sdk.asFile.absolutePath}"
        }
    )
}
