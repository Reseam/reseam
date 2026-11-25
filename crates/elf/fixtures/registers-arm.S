// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

// Each function loads its string's offset from the program counter into r0, runs one instruction,
// then adds the program counter. The lookup finds the function only when that instruction leaves
// r0 alone. Thumb reads the program counter 4 bytes ahead, ARM 8.

.syntax unified
.arch armv7-a
.fpu vfpv3-d16

.macro string key, text
    .section .rodata, "a"
\key:
    .asciz "\text"
.endm

.macro function name, set
    .text
    .\set
    .p2align 2
    .globl \name
    .type \name, %function
    .ifc \set, thumb
    .thumb_func
    .endif
\name:
.endm

.macro thumb_case name, key, text, instruction:vararg
    string \key, "\text"
    function \name, thumb
    ldr r0, 1f
    \instruction
2:  add r0, pc
    bx lr
    .p2align 2
1:  .long \key - (2b + 4)
    .size \name, . - \name
.endm

.macro arm_case name, key, text, instruction:vararg
    string \key, "\text"
    function \name, arm
    ldr r0, 1f
    \instruction
2:  add r0, pc, r0
    bx lr
1:  .long \key - (2b + 8)
    .size \name, . - \name
.endm

thumb_case thumb_kept_by_store, .Lts, "thumb store keeps the offset", str r0, [r1]
thumb_case thumb_kept_by_compare, .Ltc, "thumb compare keeps the offset", cmp r0, #1
thumb_case thumb_kept_by_vector_load, .Ltv, "thumb vector load keeps the offset", vldr d0, [r1]
thumb_case thumb_cleared_by_move, .Ltm, "thumb move clears the offset", movs r0, r1
thumb_case thumb_cleared_by_call, .Ltcall, "thumb call clears the offset", blx r1
thumb_case thumb_cleared_by_load, .Ltl, "thumb load clears the offset", ldr r0, [r1]
thumb_case thumb_cleared_by_load_multiple, .Ltlm, "thumb load multiple clears the offset", ldm r1!, {r0, r2}
thumb_case thumb_cleared_by_load_pair, .Ltlp, "thumb load pair clears the offset", ldrd r2, r0, [r1]
thumb_case thumb_cleared_by_long_multiply, .Ltlong, "thumb long multiply clears the offset", umull r2, r0, r1, r3
thumb_case thumb_cleared_by_exclusive, .Ltx, "thumb exclusive store clears the offset", strex r0, r2, [r1]
thumb_case thumb_cleared_by_vector_move, .Ltvm, "thumb vector move clears the offset", vmov r0, s0
thumb_case thumb_cleared_by_pop, .Ltp, "thumb pop clears the offset", pop {r0, r1}
thumb_case thumb_cleared_by_wide_move, .Ltw, "thumb wide move clears the offset", mov.w r0, #5
thumb_case thumb_cleared_by_shift, .Ltsh, "thumb shift clears the offset", lsl.w r0, r1, r2
thumb_case thumb_cleared_by_bitfield, .Ltb, "thumb bitfield clears the offset", ubfx r0, r1, #0, #4
thumb_case thumb_cleared_by_system, .Ltsys, "thumb system register read clears the offset", mrc p15, 0, r0, c13, c0, 3

arm_case arm_kept_by_store, .Las, "arm store keeps the offset", str r0, [r1]
arm_case arm_kept_by_compare, .Lac, "arm compare keeps the offset", cmp r0, #1
arm_case arm_cleared_by_move, .Lam, "arm move clears the offset", mov r0, r1
arm_case arm_cleared_by_call, .Lacall, "arm call clears the offset", blx r1
arm_case arm_cleared_by_load, .Lal, "arm load clears the offset", ldr r0, [r1]
arm_case arm_cleared_by_load_pair, .Lalp, "arm load pair clears the offset", ldrd r0, r1, [r2]
arm_case arm_cleared_by_multiply, .Lamul, "arm multiply clears the offset", mul r0, r1, r2
arm_case arm_cleared_by_long_multiply, .Lalong, "arm long multiply clears the offset", umull r2, r0, r1, r3
arm_case arm_cleared_by_exclusive, .Lax, "arm exclusive store clears the offset", strex r0, r2, [r1]
arm_case arm_cleared_by_vector_move, .Lavm, "arm vector move clears the offset", vmov r0, s0
arm_case arm_cleared_by_extend, .Lae, "arm extend clears the offset", uxtb r0, r1
arm_case arm_cleared_by_load_multiple, .Lalm, "arm load multiple clears the offset", ldm r1, {r0, r2}
arm_case arm_cleared_by_system, .Lasys, "arm system register read clears the offset", mrc p15, 0, r0, c13, c0, 3

// The offset as two 16-bit halves, and an address moved on by an added immediate.
    string .Ltwide, "thumb halves reach the string"
    function thumb_halves, thumb
    movw r0, #:lower16:(.Ltwide - (2f + 4))
    movt r0, #:upper16:(.Ltwide - (2f + 4))
2:  add r0, pc
    bx lr
    .size thumb_halves, . - thumb_halves

    string .Lawide, "arm halves reach the string"
    function arm_halves, arm
    movw r0, #:lower16:(.Lawide - (2f + 8))
    movt r0, #:upper16:(.Lawide - (2f + 8))
2:  add r0, pc, r0
    bx lr
    .size arm_halves, . - arm_halves

    string .Ltadd, "thumb added immediate reaches the string"
    function thumb_added, thumb
    ldr r0, 1f
2:  add r0, pc
    adds r0, #8
    bx lr
    .p2align 2
1:  .long .Ltadd - 8 - (2b + 4)
    .size thumb_added, . - thumb_added
