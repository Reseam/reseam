// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.test

import app.reseam.patch.settings.StringTable

object TestStrings :
    StringTable(
        "TestStrings",
        setOf(
            "page_media",
            "page_quality",
            "page_general",
            "page_empty",
            "section_root",
            "section_quality",
            "section_empty",
            "section_general",
            "section_gate",
            "section_first",
            "section_second",
            "root_enabled",
            "root_lines",
            "quality_first",
            "quality_second",
            "general_enabled",
            "gate_enabled",
            "first_enabled",
            "second_enabled",
        ),
    ) {
    val pageMedia = string("page_media")
    val pageQuality = string("page_quality")
    val pageGeneral = string("page_general")
    val pageEmpty = string("page_empty")
    val sectionRoot = string("section_root")
    val sectionQuality = string("section_quality")
    val sectionEmpty = string("section_empty")
    val sectionGeneral = string("section_general")
    val sectionGate = string("section_gate")
    val sectionFirst = string("section_first")
    val sectionSecond = string("section_second")
    val rootEnabled = string("root_enabled")
    val rootLines = string("root_lines")
    val qualityFirst = string("quality_first")
    val qualitySecond = string("quality_second")
    val generalEnabled = string("general_enabled")
    val gateEnabled = string("gate_enabled")
    val firstEnabled = string("first_enabled")
    val secondEnabled = string("second_enabled")
}
