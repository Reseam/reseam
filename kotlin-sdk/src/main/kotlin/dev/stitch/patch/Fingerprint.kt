package dev.stitch.patch

class Fingerprint(
    private val def: FingerprintDef,
    private val customFilter: ((MethodInfo) -> Boolean)?,
) {
    private var result: FingerprintResult? = null
    private var resolved = false

    val method: Method
        get() {
            resolve()
            return Method(result?.method ?: error("fingerprint did not match: ${def.name ?: "anonymous"}"))
        }

    val matchedCount: Int
        get() {
            resolve()
            return result?.matchedCount?.toInt() ?: 0
        }

    val matched: Boolean
        get() {
            resolve()
            return result != null
        }

    fun findAll(): List<MatchResult> {
        val results = findMethodsByFingerprint(def)
        val filtered = if (customFilter == null) results else results.filter { r ->
            val info = getMethodInfo(r.method) ?: return@filter false
            customFilter.invoke(info)
        }
        return filtered.map { MatchResult(it) }
    }

    private fun resolve() {
        if (resolved) return
        resolved = true
        if (customFilter == null) {
            result = findMethodByFingerprint(def)
        } else {
            result = findAll().firstOrNull()?.raw
        }
    }
}

class MatchResult(internal val raw: FingerprintResult) {
    val method: Method get() = Method(raw.method)
    val matchedCount: Int get() = raw.matchedCount.toInt()
}

class FingerprintBuilder {
    private var name: String? = null
    private var definingClass: String? = null
    private var accessFlags: Int? = null
    private var returnType: String? = null
    private var parameters: List<String>? = null
    private var opcodes: List<Int?>? = null
    private var strings: List<String>? = null
    private var customFilter: ((MethodInfo) -> Boolean)? = null

    fun name(name: String) { this.name = name }
    fun definingClass(descriptor: String) { this.definingClass = descriptor }
    fun accessFlags(flags: Int) { this.accessFlags = flags }
    fun returnType(type: String) { this.returnType = type }
    fun parameters(vararg params: String) { this.parameters = params.toList() }
    fun opcodes(vararg ops: Int?) { this.opcodes = ops.toList() }
    fun strings(vararg strs: String) { this.strings = strs.toList() }

    fun custom(filter: (MethodInfo) -> Boolean) {
        this.customFilter = filter
    }

    fun build(): Fingerprint = Fingerprint(
        FingerprintDef(
            name = name,
            definingClass = definingClass,
            accessFlags = accessFlags?.toUInt(),
            returnType = returnType,
            parameters = parameters,
            opcodes = opcodes?.map { it ?: -1 }?.toIntArray(),
            strings = strings,
        ),
        customFilter,
    )
}

fun fingerprint(block: FingerprintBuilder.() -> Unit): Fingerprint =
    FingerprintBuilder().apply(block).build()
