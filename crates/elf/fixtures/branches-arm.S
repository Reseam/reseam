// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

// Each function returns 1 when its conditional branch jumps and 2 when it falls through. The
// branch tests the first argument, so the results for 0 and 1 differ.

.syntax unified
.arch armv7-a

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

.macro case name, set, test, branch
    function \name, \set
    \test
    \branch
    movs r0, #2
    bx lr
1:  movs r0, #1
    bx lr
    .size \name, . - \name
.endm

case thumb_branch_narrow, thumb, "cmp r0, #0", "bne.n 1f"
case thumb_branch_wide, thumb, "cmp r0, #0", "bne.w 1f"
case thumb_branch_cbz, thumb, "nop", "cbz r0, 1f"
case thumb_branch_cbnz, thumb, "nop", "cbnz r0, 1f"
case arm_branch, arm, "cmp r0, #0", "bne 1f"

.macro back name, set, branch
    function \name, \set
    b 2f
1:  movs r0, #1
    bx lr
2:  cmp r0, #0
    \branch
    movs r0, #2
    bx lr
    .size \name, . - \name
.endm

back thumb_branch_narrow_back, thumb, "bne.n 1b"
back thumb_branch_wide_back, thumb, "bne.w 1b"
back arm_branch_back, arm, "bne 1b"

    function thumb_branch_twice, thumb
    cmp r0, #0
    bne.n 1f
    cbz r1, 1f
    movs r0, #2
    bx lr
1:  movs r0, #1
    bx lr
    .size thumb_branch_twice, . - thumb_branch_twice
