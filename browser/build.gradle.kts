// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

plugins {
    java
    kotlin("jvm")
}

repositories { mavenCentral() }

dependencies { implementation("com.google.code.gson:gson:2.13.2") }

java { toolchain { languageVersion.set(JavaLanguageVersion.of(17)) } }

sourceSets { main { java.srcDir("java") } }

tasks.register<Jar>("hostJar") {
    archiveFileName.set("browser-host.jar")
    destinationDirectory.set(layout.buildDirectory.dir("runtime"))
    duplicatesStrategy = DuplicatesStrategy.EXCLUDE
    from(sourceSets.main.get().output)
    from(configurations.runtimeClasspath.get().map { if (it.isDirectory) it else zipTree(it) })
    exclude("META-INF/*.SF", "META-INF/*.RSA", "META-INF/*.DSA")
}

// The patch bytecode stays unchanged. Only the generated native transport uses
// one-element long arrays to avoid LiveConnect's Number conversion.
kotlin {
    jvmToolchain(17)
    sourceSets.main { kotlin.srcDir(rootProject.file("patch-api/generated/browser/kotlin")) }
}

tasks.register<Jar>("browserRuntimeJar") {
    dependsOn(":reseam-sdk:patchRuntimeJar", tasks.named("compileKotlin"))
    archiveFileName.set("reseam-runtime.jar")
    destinationDirectory.set(layout.buildDirectory.dir("runtime"))
    from({ zipTree(rootProject.file("build/runtime/reseam-runtime.jar")) }) {
        exclude("app/reseam/patch/native/Native.class")
    }
    from(layout.buildDirectory.dir("classes/kotlin/main")) {
        include("app/reseam/patch/native/Native.class")
    }
}
