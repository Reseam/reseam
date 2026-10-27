// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

package app.reseam.patch.search

import app.reseam.patch.ClassRankScope
import app.reseam.patch.MethodRankScope
import app.reseam.patch.RankScope
import app.reseam.patch.Type
import app.reseam.patch.descriptor
import app.reseam.patch.dex.DexClass
import app.reseam.patch.dex.Method
import app.reseam.patch.dex.descriptor
import app.reseam.patch.dex.parameterTypes

internal open class RankScopeImpl(protected val index: SearchIndex, override val type: String) :
    RankScope {
    override fun methods(proto: String): List<Method> =
        index.classFor(type)?.methods?.filter { it.proto == proto }.orEmpty()

    override fun zeroArgListGetters(): Int =
        index.classFor(type)?.methods?.count { it.proto == "()${Type.List}" } ?: 0
}

internal class MethodRankScopeImpl(index: SearchIndex, override val method: Method) :
    RankScopeImpl(index, method.owner), MethodRankScope {
    override val paramCount: Int
        get() = method.parameterTypes.size

    override fun callSitesFollowedByCast(type: String, lookAhead: Int): Int =
        index.references.followedByCheckCast(method, descriptor(type), lookAhead)
}

internal class ClassRankScopeImpl(index: SearchIndex, override val classDef: DexClass) :
    RankScopeImpl(index, classDef.descriptor), ClassRankScope
