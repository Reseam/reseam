// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.gradle

import java.nio.file.Files
import java.nio.file.Path
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.SerialKind
import kotlinx.serialization.json.Json

internal const val PATCH_INDEX = "META-INF/reseam/patches.json"
internal val bundleJson = Json { ignoreUnknownKeys = true }

@Serializable
internal enum class MemberKind {
    @SerialName("field") FIELD,
    @SerialName("method") METHOD,
}

@Serializable
internal data class PatchDeclaration(
    @SerialName("class") val className: String,
    val owner: String,
    val member: String,
    val kind: MemberKind,
    val id: String,
)

internal fun main(args: Array<String>) {
    val rust = buildString {
        append("// Generated from PatchIndexFormat.kt. Do not edit.\n\n")
        append("pub(crate) const PATCH_INDEX: &str = \"$PATCH_INDEX\";\n\n")
        val kind = MemberKind.serializer().descriptor
        append(
            "#[cfg(feature = \"kotlin\")]\n#[derive(Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]\npub(crate) enum MemberKind {\n"
        )
        for (index in 0 until kind.elementsCount) {
            val name = kind.getElementName(index)
            append(
                "    #[serde(rename = \"$name\")]\n    ${name.replaceFirstChar(Char::uppercase)},\n"
            )
        }
        append(
            "}\n\n#[cfg(feature = \"kotlin\")]\n#[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq)]\npub(crate) struct Declaration {\n"
        )
        val declaration = PatchDeclaration.serializer().descriptor
        for (index in 0 until declaration.elementsCount) {
            val name = declaration.getElementName(index)
            val field = if (name == "class") "class_name" else name
            val type =
                when (declaration.getElementDescriptor(index).kind) {
                    PrimitiveKind.STRING -> "String"
                    SerialKind.ENUM -> "MemberKind"
                    else -> error("unsupported index field $name")
                }
            if (name != field) append("    #[serde(rename = \"$name\")]\n")
            append("    pub(crate) $field: $type,\n")
        }
        append("}\n")
    }
    val output = Path.of(args.single())
    if (!Files.exists(output) || Files.readString(output) != rust) {
        if (System.getProperty("reseam.updateIndexBindings") == "true")
            Files.writeString(output, rust)
        else
            error(
                "patch index Rust bindings are stale; run :reseam-gradle-plugin:generatePatchIndexBindings -Dreseam.updateIndexBindings=true"
            )
    }
}
