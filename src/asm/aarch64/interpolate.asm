// x0 input cursor; x1 input end; x2 descriptor array; x3 count.
// x4 output cursor; x5 output end; x6 original output pointer.
// Descriptor fields are key pointer, key length, value pointer, value length.
// x7-x17 are scratch. x18 is never used. No stack or external calls.
mov x6, x4
2:
    cmp x0, x1
    b.eq 20f
    ldrb w14, [x0]
    cmp w14, #37
    b.eq 3f
    cmp x4, x5
    b.eq 21f
    strb w14, [x4], #1
    add x0, x0, #1
    b 2b
3:
    mov x7, x0
    add x8, x0, #1
    cmp x8, x1
    b.eq 21f
    ldrb w14, [x8]
    cmp w14, #123
    b.ne 21f
    add x8, x8, #1
    mov x9, x8
4:
    cmp x9, x1
    b.eq 21f
    ldrb w14, [x9]
    cmp w14, #125
    b.eq 5f
    cmp w14, #37
    b.eq 21f
    add x9, x9, #1
    b 4b
5:
    sub x12, x9, x8
    mov x10, x2
    mov x11, x3
6:
    cbz x11, 10f
    ldr x16, [x10, #8]
    cmp x16, x12
    b.ne 8f
    ldr x15, [x10]
    mov x13, #0
7:
    cmp x13, x12
    b.eq 9f
    ldrb w14, [x8, x13]
    ldrb w17, [x15, x13]
    cmp w14, w17
    b.ne 8f
    add x13, x13, #1
    b 7b
8:
    add x10, x10, #32
    sub x11, x11, #1
    b 6b
9:
    ldr x15, [x10, #16]
    ldr x16, [x10, #24]
    b 11f
10:
    mov x15, x7
    sub x16, x9, x7
    add x16, x16, #1
11:
    sub x17, x5, x4
    cmp x16, x17
    b.hi 21f
12:
    cbz x16, 13f
    ldrb w14, [x15], #1
    strb w14, [x4], #1
    sub x16, x16, #1
    b 12b
13:
    add x0, x9, #1
    b 2b
20:
    sub x0, x4, x6
    b 22f
21:
    mov x0, #-1
22:
