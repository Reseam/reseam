// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.test

import app.reseam.patch.XmlDocument
import app.reseam.patch.invoke

val xmlAttributeBinding =
    fixturePatch("xml-attribute-binding") {
        manifest.edit {
            root.appendChild(
                createElement("intent").apply {
                    this["android:targetPackage"] = "com.example.target"
                    this["android:targetClass"] = "com.example.target.Settings"
                }
            )
            val unknown = runCatching { root["android:notAnAttribute"] = "x" }
            check(unknown.isFailure)
            val undeclared = runCatching { root["yt:layout_constraintLeft_toLeftOf"] = "x" }
            check(undeclared.isFailure)
        }
    }

val xmlGraft =
    declaredFixture("xml-graft") {
        enabledByDefault(false)
        dependsOn(resourceEntries)

        execute {
            XmlDocument.compile(CONTROL_FRAGMENT).use { fragment ->
                resources.editXml("layout", "player_controls") {
                    root.appendChild(adopt(fragment.root))
                    findByTag("ImageView").first()["app:layout_constraintRight_toLeftOf"] =
                        "@id/controls"
                }
                manifest.edit {
                    val foreign = runCatching { adopt(fragment.root) }
                    check(foreign.isFailure)
                    declareNamespace("app", RES_AUTO)
                    root["app:layout_constraintRight_toLeftOf"] = "@id/controls"
                }
            }
        }
    }

val xmlFilesWith =
    fixturePatch("xml-files-with") {
        val found =
            listOf(
                files.xmlFilesWith("android:layout_width"),
                files.xmlFilesWith("android:targetPackage", "app:missing"),
            )
        files.write(
            "assets/xml-files-with.txt",
            found.joinToString("|") { it.joinToString(",") }.encodeToByteArray(),
        )
    }
