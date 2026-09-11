// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

package app.reseam.test

import app.reseam.patch.Type
import app.reseam.patch.after
import app.reseam.patch.klass
import app.reseam.patch.method
import app.reseam.patch.patch

val finalizeOwner = patch("finalize-owner") {
    description("Exercises afterDependents through the real Kotlin runtime")
    compatibleWith("com.example.test")

    execute {
        log.info("finalize-owner execute")
    }

    afterDependents {
        manifest.addPermission("android.permission.INTERNET")
    }
}

val runtimeApi = patch("runtime-api") {
    description("Exercises PatchRuntime scopes against split APK state")
    compatibleWith("com.example.test")
    val baseVersion = stringOption("baseVersion", default = "2.0-base")
    val splitVersion = stringOption("splitVersion", default = "2.0-split")
    val splitText = stringOption("splitText", default = "Split patched")

    execute {
        manifest.setVersionName(options[baseVersion])
        manifest.component("config.test").setVersionName(options[splitVersion])
        resources.setString("split_label", options[splitText])
        files.write("assets/base-marker.txt", "base".encodeToByteArray())
        files.component("config.test").write("assets/split-marker.txt", "split".encodeToByteArray())
    }
}

val dependentRuntime = patch("dependent-runtime") {
    description("Depends on finalize-owner to trigger afterDependents")
    compatibleWith("com.example.test")
    dependsOn(finalizeOwner)

    execute {
        files.component("config.test").write("assets/dependent-marker.txt", "dependent".encodeToByteArray())
    }
}

val requiredOption = patch("required-option") {
    description("Used to verify option validation against real Kotlin patches")
    compatibleWith("com.example.test")
    enabledByDefault(false)
    val token = stringOption("token", required = true)

    execute {
        log.info(options[token])
    }
}

val internalHelper = patch {
    description("An internal dependency: never listed, runs when something depends on it")
    compatibleWith("com.example.test")

    execute {
        files.write("assets/internal-marker.txt", "internal".encodeToByteArray())
    }
}

val usesInternal = patch("uses-internal") {
    description("Pulls the internal helper in as a dependency")
    compatibleWith("com.example.test")
    dependsOn(internalHelper)

    execute {
        log.info("uses-internal execute")
    }
}

val afterEntryValues = patch("after-entry-values") {
    description("Checks entry argument lifetimes through the real code emitter")
    compatibleWith("com.example.test")
    enabledByDefault(false)

    execute {
        val target = klass("com.example.HookTarget")
        check(target.method("invokeGrowth").method.growLocalRegisters(10))
        target.method("invokeGrowth").after {
            callStatic("com.example.Observer", "entry", "(I)V", param(3))
        }
        val receiver = target.method("receiver").method
        val noRegister = runCatching { receiver.findFreeRegister(0, (0 until receiver.registersSize).toList()) }
        check(noRegister.exceptionOrNull()?.message?.contains("No free register") == true)
        target.method("temporaryReuse").after {
            val held = long(0x123456789abcdef0L)
            repeat(32) {
                callStatic("com.example.Observer", "wide", "(J)V", long(it.toLong()))
                callStatic("com.example.Observer", "scalar", "(I)V", int(it))
            }
            whenTrue(param(0)) {
                callStatic("com.example.Observer", "wide", "(J)V", held)
            } otherwise {
                callStatic("com.example.Observer", "wide", "(J)V", held)
            }
            capture("result").assign(held)
        }
        target.method("getFeatureSwitchValue").after {
            val marker = int(42)
            callStatic("com.example.Observer", "record", "(ILjava/lang/String;JDLjava/lang/String;Ljava/lang/String;Ljava/lang/Object;)V",
                marker, param(0), paramOfType(Type.Long), param(2), lastParam, paramOfType(Type.String), capture("result"))
        }
        target.method("receiver").after {
            callStatic("com.example.Observer", "receiver", "(Lcom/example/HookTarget;)V", thisObject)
        }
        target.method("resultOnly").after {
            capture("result").assign(int(42))
        }
    }
}
