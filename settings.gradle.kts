// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

pluginManagement {
    repositories {
        google {
            mavenContent {
                includeGroupAndSubgroups("androidx")
                includeGroupAndSubgroups("com.android")
                includeGroupAndSubgroups("com.google")
            }
        }
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositories {
        google {
            mavenContent {
                includeGroupAndSubgroups("androidx")
                includeGroupAndSubgroups("com.android")
                includeGroupAndSubgroups("com.google")
            }
        }
        mavenCentral()
    }
}

rootProject.name = "reseam"

include(":patch-api", ":sdk-kotlin", ":gradle-plugin", ":browser")

project(":patch-api").name = "reseam-patch-sdk"

project(":sdk-kotlin").name = "reseam-sdk"

project(":gradle-plugin").name = "reseam-gradle-plugin"

project(":browser").name = "reseam-browser-host"
