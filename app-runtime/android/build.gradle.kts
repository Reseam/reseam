// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

plugins {
    id("app.reseam.extension")
    `maven-publish`
}

dependencies { compileOnly("com.facebook.react:react-android:0.86.0") }

reseamArtifact { kotlinPackage = "app.reseam.runtime.kotlin" }

reseamStrings {
    objectName = "app.reseam.runtime.RuntimeStrings"
    tableId = "runtime"
    assetDirectory = "reseam/runtime/strings"
}

publishing { publications { create<MavenPublication>("maven") { from(components["java"]) } } }
