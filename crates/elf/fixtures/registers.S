// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

// Each function loads the page of its string into x8, runs one instruction, then adds the
// string's offset in the page. The lookup finds the function only when that instruction leaves
// x8 alone. The instructions sit between adrp and add, so the linker keeps both.

.arch armv8.5-a

.macro case name, key, text, instruction:vararg
    .section .rodata, "a"
\key:
    .asciz "\text"
    .text
    .p2align 2
    .type \name, %function
\name:
    .cfi_startproc
    bti c
    adrp x8, \key
    \instruction
    add x0, x8, :lo12:\key
    ret
    .cfi_endproc
    .size \name, . - \name
.endm

case kept_by_store, .Lstore, "store keeps the page", str x8, [x1]
case kept_by_store_pair, .Lpair, "pre-indexed pair store keeps the page", stp x8, x9, [sp, #-16]!
case kept_by_vector_load, .Lvector, "vector load keeps the page", ldr q8, [x1]
case cleared_by_move, .Lmove, "move clears the page", mov x8, x1
case cleared_by_call, .Lcall, "call clears the page", blr x1
case cleared_by_literal, .Lliteral, "literal load clears the page", ldr x8, .
case cleared_by_atomic, .Latomic, "atomic add clears the page", ldadd x1, x8, [x2]
case cleared_by_exclusive, .Lexclusive, "exclusive store status clears the page", stxr w8, x1, [x2]
case cleared_by_system, .Lsystem, "system register read clears the page", mrs x8, tpidr_el0
