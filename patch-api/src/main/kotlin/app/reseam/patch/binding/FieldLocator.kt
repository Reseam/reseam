// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.binding

import app.reseam.patch.FieldLocator
import app.reseam.patch.FieldTarget
import app.reseam.patch.RankScope
import app.reseam.patch.descriptor
import app.reseam.patch.dex.descriptor
import app.reseam.patch.native.indexOfFirstString
import app.reseam.patch.search.RankScopeImpl
import app.reseam.patch.search.Ranker
import app.reseam.patch.search.SearchIndex
import app.reseam.patch.types.FieldRef

private enum class ObjectReadScope {
    DECLARING_CLASS,
    ANY_CLASS,
}

internal class FieldLocatorCompiler(
    private val index: SearchIndex,
    private val defaultOwner: String?,
) : FieldLocator {
    private var ownerDescriptor: String? = null
    private var objectReadScope: ObjectReadScope? = null
    private var nearestBeforeString: String? = null
    private val rankers = mutableListOf<Ranker<RankScope>>()
    private var minScore = Int.MIN_VALUE

    override fun owner(type: String) {
        ownerDescriptor = descriptor(type)
    }

    override fun firstObjectRead() {
        objectReadScope = ObjectReadScope.DECLARING_CLASS
    }

    override fun firstObjectReadAnyOwner() {
        objectReadScope = ObjectReadScope.ANY_CLASS
    }

    override fun nearestObjectReadBeforeString(value: String) {
        nearestBeforeString = value
    }

    override fun rankBy(label: String, block: RankScope.() -> Int) {
        rankers += Ranker(label, block)
    }

    override fun requireScoreAtLeast(score: Int) {
        minScore = score
    }

    fun resolveFromAnchor(label: String): FieldTarget {
        val owner =
            ownerDescriptor ?: defaultOwner ?: error("Field locator '$label' requires owner(...)")
        val ownerClass = index.requireClass(owner)
        nearestBeforeString?.let { needle ->
            val nearest =
                ownerClass.methods
                    .mapNotNull { method ->
                        val anchor = method.indexOfFirstString(needle) ?: return@mapNotNull null
                        method.instructions.objectRead(anchor - 1 downTo 0, owner)?.let {
                            anchor - it.index to it.value
                        }
                    }
                    .minByOrNull { it.first }
                    ?: error("No object field read before '$needle' on $owner")
            return FieldTarget.of(nearest.second, label)
        }
        objectReadScope?.let { scope ->
            val fieldOwner =
                when (scope) {
                    ObjectReadScope.DECLARING_CLASS -> owner
                    ObjectReadScope.ANY_CLASS -> null
                }
            val read =
                ownerClass.methods.firstNotNullOfOrNull {
                    it.instructions.let { insns -> insns.objectRead(insns.indices, fieldOwner) }
                } ?: error("No object field read found on $owner")
            return FieldTarget.of(read.value, label)
        }
        return resolveOnType(label)
    }

    fun resolveOnType(label: String): FieldTarget {
        val owner =
            ownerDescriptor
                ?: defaultOwner
                ?: error("No owner type is available for field '$label'")
        val ownerClass = index.requireClass(owner)
        val candidates =
            ownerClass.instanceFields.map { field ->
                FieldRef(owner, field.name, field.fieldType) to
                    rankers.sumOf { it.block(RankScopeImpl(index, field.fieldType)) }
            }
        require(candidates.isNotEmpty()) { "No instance fields found on $owner" }
        val winner = candidates.maxBy { it.second }
        require(winner.second >= minScore) {
            "Best field candidate for '$label' on $owner scored ${winner.second}, below required $minScore"
        }
        if (rankers.isEmpty())
            require(candidates.size == 1) {
                "Field '$label' on $owner is ambiguous without ranking; found ${candidates.size} candidates"
            }
        return FieldTarget.of(winner.first, label)
    }
}
