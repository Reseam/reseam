// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

import org.gradle.api.credentials.HttpHeaderCredentials
import org.gradle.authentication.http.HttpHeaderAuthentication

group = "app.reseam"

version = providers.gradleProperty("reseamSdkVersion").orElse("0.0.0-local").get().removePrefix("v")

plugins.withId("maven-publish") {
    configure<PublishingExtension> {
        repositories {
            maven {
                name = "Forgejo"
                url = uri("https://git.reseam.app/api/packages/reseam/maven")
                credentials(HttpHeaderCredentials::class) {
                    name = "Authorization"
                    value =
                        providers
                            .environmentVariable("FORGEJO_PACKAGES_TOKEN")
                            .map { "token $it" }
                            .orElse("")
                            .get()
                }
                authentication { create<HttpHeaderAuthentication>("header") }
            }
        }
    }
}
