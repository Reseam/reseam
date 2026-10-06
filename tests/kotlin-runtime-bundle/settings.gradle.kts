// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

pluginManagement { includeBuild("../..") }

includeBuild("../..") {
    dependencySubstitution {
        substitute(module("app.reseam:reseam-patch-sdk")).using(project(":reseam-patch-sdk"))
    }
}

rootProject.name = "reseam-test-patches"

includeBuild("../..")
