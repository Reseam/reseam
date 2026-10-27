// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.test

import app.reseam.patch.ResourceValue
import app.reseam.patch.invoke

val resourceEntries =
    fixturePatch("resource-entries") {
        val icon = resources.addFile("drawable", "reseam_icon", ICON_PATH, ICON_BYTES)
        check(resources.id("drawable", "reseam_icon") == icon) {
            "drawable/reseam_icon is not addressable"
        }
        resources.addFile("mipmap", "reseam_launcher", "res/reseam_launcher.png", ICON_BYTES)
        check(resources.path("mipmap", "reseam_launcher") == "res/reseam_launcher.png") {
            "mipmap path"
        }

        resources.addFile(
            "drawable",
            "reseam_pulse",
            "res/drawable/reseam_pulse.xml",
            PULSE_XML.encodeToByteArray(),
        )

        check(runCatching { resources.addFile("drawable", "absent", "res/absent.png") }.isFailure)
        check(runCatching { resources.path("string", "base_label") }.isFailure)

        resources.style("Theme.Test") {
            this["android:windowBackground"] = "@drawable/reseam_icon"
            this["android:textColor"] = "#ff102030"
        }
        resources.style("Theme.Reseam", parent = "@style/Theme.Test") {
            this["layout_constraintRight_toLeftOf"] = "@id/controls"
        }
        val orphan = runCatching {
            resources.style("Theme.Missing") { this["android:textColor"] = "#ffffffff" }
        }
        check(orphan.isFailure)

        val lengths = resources.getArray("double_tap_lengths")
        resources.setArray(
            "double_tap_lengths",
            lengths + listOf("15", "20", "@drawable/reseam_icon"),
        )
        resources.setStringArray(
            "double_tap_lengths",
            resources.getArray("double_tap_lengths") + listOf("true", "@string/missing"),
        )

        val typed =
            resources.getArrayValues("double_tap_lengths") +
                listOf(
                    ResourceValue(0x04u, 0x80000000u),
                    ResourceValue(0x05u, 0x00002101u),
                    ResourceValue(0x12u, 1u),
                    ResourceValue(0x01u, icon),
                )
        resources.setArrayValues("double_tap_lengths", typed)

        resources
            .component("config.test")
            .addFile(
                "xml",
                "split_settings",
                "res/split.xml",
                "<settings />".encodeToByteArray(),
            )
        files.write("res/split.xml", "<base />".encodeToByteArray())
        resources.editXml("xml", "split_settings") { root.appendChild(createElement("split")) }

        files.write(
            "assets/resource-entries.txt",
            "${resources.path("layout", "player_controls")}|${lengths.joinToString(",")}"
                .encodeToByteArray(),
        )
    }
