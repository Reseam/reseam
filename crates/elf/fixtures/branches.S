// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

// Each function returns 1 when its conditional branch jumps and 2 when it falls through. The
// branch tests the first argument, so the results for 0 and 1 differ.

.arch armv8.8-a

.macro function name
    .text
    .p2align 2
    .globl \name
    .type \name, %function
\name:
    .cfi_startproc
    bti c
.endm

.macro end name
    .cfi_endproc
    .size \name, . - \name
.endm

.macro case name, test, branch
    function \name
    \test
    \branch
    mov w0, #2
    ret
1:  mov w0, #1
    ret
    end \name
.endm

case branch_ne, "cmp w0, #0", "b.ne 1f"
case branch_bc, "cmp w0, #0", "bc.ne 1f"
case branch_cbz, "nop", "cbz w0, 1f"
case branch_cbnz, "nop", "cbnz x0, 1f"
case branch_tbz, "nop", "tbz w0, #0, 1f"
case branch_tbnz, "nop", "tbnz x0, #0, 1f"

    function branch_back
    b 2f
1:  mov w0, #1
    ret
2:  cmp w0, #0
    b.ne 1b
    mov w0, #2
    ret
    end branch_back

    function branch_twice
    cmp w0, #0
    b.ne 1f
    cbz w1, 1f
    mov w0, #2
    ret
1:  mov w0, #1
    ret
    end branch_twice
