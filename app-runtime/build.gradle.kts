// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

subprojects { apply(from = rootDir.resolve("../gradle/publishing.gradle.kts")) }

evaluationDependsOnChildren()

val dex = project(":reseam-app-runtime").tasks.named("dex")
val hermes = project(":reseam-app-runtime-js").tasks.named("hermes98")

tasks.register<Sync>("stage") {
    description = "Collects the app runtime's DEX and Hermes bytecode for the engine to embed."
    from(dex) {
        include("classes.dex")
        rename { "reseam-app-runtime.dex" }
    }
    from(hermes) {
        include("*.hbc")
        into("hermes/v98")
    }
    from(project(":reseam-app-runtime").tasks.named("generateStrings")) {
        include("reseam/strings/runtime/**")
        eachFile { path = "strings/" + path.removePrefix("reseam/strings/runtime/") }
        includeEmptyDirs = false
    }
    into(rootDir.resolve("../build/runtime/app"))
    // The engine links this DEX alone, so d8 must not leave definitions in a shared globals file.
    val globals = files(dex).asFileTree.matching { include("**/*.globals") }
    doFirst {
        if (!globals.isEmpty) {
            throw GradleException("the app runtime needs d8 global synthetics: ${globals.files}")
        }
    }
}
