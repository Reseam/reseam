// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

@file:Suppress("unused")

package app.reseam.patch

typealias Predicate<T> = T.() -> Boolean

typealias IndexedMatcherPredicate<T> = T.(lastMatchedIndex: Int, currentIndex: Int, setNextIndex: (Int?) -> Unit) -> Boolean

fun matchInstructionSequence(
    instructions: List<Instruction>,
    predicates: List<IndexedMatcherPredicate<Instruction>>,
): List<Int>? {
    if (predicates.isEmpty()) return emptyList()

    data class Frame(
        val patternIndex: Int,
        val lastMatchedIndex: Int,
        val previousFrame: Frame?,
        var nextHayIndex: Int,
        val matchedIndex: Int,
    )

    val stack = ArrayDeque<Frame>()
    stack.add(Frame(0, -1, null, 0, -1))
    var nextIndex: Int? = null

    while (stack.isNotEmpty()) {
        val frame = stack.last()

        if (frame.nextHayIndex >= instructions.size || nextIndex == -1) {
            stack.removeLast()
            nextIndex = null
            continue
        }

        val i = frame.nextHayIndex
        nextIndex = null

        if (predicates[frame.patternIndex](
                instructions[i],
                frame.lastMatchedIndex,
                i,
            ) { nextIndex = it }
        ) {
            val newFrame = Frame(
                patternIndex = frame.patternIndex + 1,
                lastMatchedIndex = i,
                previousFrame = frame,
                nextHayIndex = i + 1,
                matchedIndex = i,
            )
            if (newFrame.patternIndex == predicates.size) {
                return buildList(predicates.size) {
                    var f: Frame? = newFrame
                    while (f != null && f.matchedIndex != -1) {
                        add(f.matchedIndex)
                        f = f.previousFrame
                    }
                }.asReversed()
            }
            stack.add(newFrame)
        }

        frame.nextHayIndex = when (val ni = nextIndex) {
            null -> frame.nextHayIndex + 1
            -1 -> 0
            else -> ni
        }
    }

    return null
}

// region Sequential combinators

fun <T> after(
    range: IntRange = 1..1,
    predicate: IndexedMatcherPredicate<T>,
): IndexedMatcherPredicate<T> =
    predicate@{ lastMatchedIndex, currentIndex, setNextIndex ->
        val distance = currentIndex - lastMatchedIndex
        setNextIndex(
            when {
                distance < range.first -> lastMatchedIndex + range.first
                distance > range.last -> -1
                else -> return@predicate predicate(lastMatchedIndex, currentIndex, setNextIndex)
            },
        )
        false
    }

fun <T> after(
    range: IntRange = 1..1,
    predicate: Predicate<T>,
) = after<T>(range) { _, _, _ -> predicate() }

fun <T> after(predicate: IndexedMatcherPredicate<T>) =
    after<T>(1..1) { lastMatchedIndex, currentIndex, setNextIndex ->
        predicate(lastMatchedIndex, currentIndex, setNextIndex)
    }

fun <T> after(predicate: Predicate<T>) = after<T> { _, _, _ -> predicate() }

fun <T> afterAtMost(
    steps: Int = 1,
    predicate: IndexedMatcherPredicate<T>,
) = after<T>(1..steps) { lastMatchedIndex, currentIndex, setNextIndex ->
    predicate(lastMatchedIndex, currentIndex, setNextIndex)
}

fun <T> afterAtMost(
    steps: Int = 1,
    predicate: Predicate<T>,
) = after<T>(1..steps) { _, _, _ -> predicate() }

fun <T> after(
    steps: Int = 1,
    predicate: IndexedMatcherPredicate<T>,
) = after<T>(steps..steps) { lastMatchedIndex, currentIndex, setNextIndex ->
    predicate(lastMatchedIndex, currentIndex, setNextIndex)
}

fun <T> after(
    steps: Int = 1,
    predicate: Predicate<T>,
) = after<T>(steps..steps) { _, _, _ -> predicate() }

// endregion

// region Combinators

fun <T> allOf(vararg predicates: IndexedMatcherPredicate<T>): IndexedMatcherPredicate<T> =
    { lastMatchedIndex, currentIndex, setNextIndex ->
        predicates.all { predicate -> predicate(lastMatchedIndex, currentIndex, setNextIndex) }
    }

fun <T> anyOf(vararg predicates: IndexedMatcherPredicate<T>): IndexedMatcherPredicate<T> =
    { lastMatchedIndex, currentIndex, setNextIndex ->
        predicates.any { predicate -> predicate(lastMatchedIndex, currentIndex, setNextIndex) }
    }

fun <T> noneOf(vararg predicates: IndexedMatcherPredicate<T>): IndexedMatcherPredicate<T> =
    { lastMatchedIndex, currentIndex, setNextIndex ->
        predicates.none { predicate -> predicate(lastMatchedIndex, currentIndex, setNextIndex) }
    }

fun <T> unorderedAllOf(vararg predicates: IndexedMatcherPredicate<T>): Array<IndexedMatcherPredicate<T>> {
    val usedPredicateIndices = mutableListOf<Int>()
    var lastPatternIndex = -1

    return predicates.indices
        .map<Int, IndexedMatcherPredicate<T>> { patternIndex ->
            predicate@{ lastMatchedIndex, currentIndex, setNextIndex ->
                if (patternIndex <= lastPatternIndex) {
                    while (usedPredicateIndices.size > patternIndex) {
                        usedPredicateIndices.removeAt(usedPredicateIndices.lastIndex)
                    }
                }
                lastPatternIndex = patternIndex

                for (predicateIndex in predicates.indices) {
                    if (predicateIndex in usedPredicateIndices) continue
                    if (predicates[predicateIndex](lastMatchedIndex, currentIndex) { nextIndex ->
                            if (nextIndex != -1) setNextIndex(Int.MAX_VALUE)
                        }
                    ) {
                        usedPredicateIndices += predicateIndex
                        return@predicate true
                    }
                }
                false
            }
        }.toTypedArray()
}

// endregion

// region Instruction predicates

fun method(predicate: Predicate<MethodRef> = { true }): IndexedMatcherPredicate<Instruction> =
    { _, _, _ -> methodRef()?.predicate() == true }

fun method(
    name: String,
    compare: String.(String) -> Boolean = String::equals,
) = method { this.name.compare(name) }

fun field(predicate: Predicate<FieldRef> = { true }): IndexedMatcherPredicate<Instruction> =
    { _, _, _ -> fieldRef()?.predicate() == true }

fun field(
    name: String,
    compare: String.(String) -> Boolean = String::equals,
) = field { this.name.compare(name) }

fun type(predicate: Predicate<String> = { true }): IndexedMatcherPredicate<Instruction> =
    { _, _, _ -> typeRef()?.predicate() == true }

fun type(
    type: String,
    compare: String.(type: String) -> Boolean = type.typeComparer(),
) = type { compare(type) }

fun string(predicate: Predicate<String> = { true }): IndexedMatcherPredicate<Instruction> =
    { _, _, _ -> stringRef()?.predicate() == true }

fun string(
    s: String,
    compare: String.(String) -> Boolean = String::equals,
) = string { compare(s) }

fun literal(predicate: Predicate<Long> = { true }): IndexedMatcherPredicate<Instruction> =
    { _, _, _ -> (this is Instruction.RegLiteral) && this.value0.literal.predicate() }

fun literal(
    value: Long,
    compare: Long.(Long) -> Boolean = Long::equals,
) = literal { compare(value) }

// endregion

// region Extension operators for DSL

operator fun String.invoke(compare: String.(String) -> Boolean = String::equals): IndexedMatcherPredicate<Instruction> =
    string(this, compare)

operator fun Long.invoke(compare: Long.(Long) -> Boolean = Long::equals): IndexedMatcherPredicate<Instruction> =
    literal(this, compare)

operator fun Int.invoke(): IndexedMatcherPredicate<Instruction> =
    { _, _, _ -> opcode() == this@invoke }

// endregion

// region Type comparer

internal fun String.typeComparer(): String.(String) -> Boolean {
    val primitiveTypes = setOf("V", "Z", "B", "S", "C", "I", "J", "F", "D")
    return when {
        this in primitiveTypes -> String::equals
        startsWith("L") && endsWith(";") -> String::equals
        startsWith("[") -> String::equals
        startsWith("L") -> String::startsWith
        endsWith(";") -> String::endsWith
        else -> String::contains
    }
}

// endregion
