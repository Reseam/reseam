// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
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
