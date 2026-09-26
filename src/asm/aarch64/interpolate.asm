// x0 input cursor; x1 input end; x2 descriptor array; x3 count.
// x4 output cursor; x5 output end; x6 original output pointer.
// Descriptor fields are key pointer, key length, value pointer, value length.
// x7-x17 are scratch. x18 is never used. No stack or external calls.
// v0 stores copy data, v1 stores percent bytes, and v2 stores comparison bits.
mov x6, x4
movi v1.16b, #37
2:
    cmp x0, x1
    b.eq 20f
    sub x17, x1, x0
    cmp x17, #16
    b.lo 31f
    sub x17, x5, x4
    cmp x17, #16
    b.lo 31f
    ldr q0, [x0]
    cmeq v2.16b, v0.16b, v1.16b
    umaxv b2, v2.16b
    umov w14, v2.b[0]
    cbz x14, 30f
    mov x14, #0
32:
    cmp x14, #16
    b.eq 21f
    ldrb w17, [x0, x14]
    cmp w17, #37
    b.eq 33f
    add x14, x14, #1
    b 32b
33:
    cbz x14, 3f
    mov x15, x0
    mov x16, x14
    add x9, x0, x14
    sub x9, x9, #1
    b 11f
30:
    str q0, [x4], #16
    add x0, x0, #16
    b 2b
31:
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
    sub x16, x12, x13
    cmp x16, #8
    b.lo 40f
    ldr x14, [x8, x13]
    ldr x17, [x15, x13]
    cmp x14, x17
    b.ne 8f
    add x13, x13, #8
    b 7b
40:
    cmp x16, #4
    b.lo 41f
    ldr w14, [x8, x13]
    ldr w17, [x15, x13]
    cmp w14, w17
    b.ne 8f
    add x13, x13, #4
    sub x16, x16, #4
41:
    cmp x16, #2
    b.lo 42f
    ldrh w14, [x8, x13]
    ldrh w17, [x15, x13]
    cmp w14, w17
    b.ne 8f
    add x13, x13, #2
    sub x16, x16, #2
42:
    cbz x16, 9f
    ldrb w14, [x8, x13]
    ldrb w17, [x15, x13]
    cmp w14, w17
    b.ne 8f
    b 9f
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
    cmp x16, #16
    b.lo 43f
    ldr q0, [x15], #16
    str q0, [x4], #16
    sub x16, x16, #16
    b 12b
43:
    cmp x16, #8
    b.lo 44f
    ldr x14, [x15], #8
    str x14, [x4], #8
    sub x16, x16, #8
44:
    cmp x16, #4
    b.lo 45f
    ldr w14, [x15], #4
    str w14, [x4], #4
    sub x16, x16, #4
45:
    cmp x16, #2
    b.lo 46f
    ldrh w14, [x15], #2
    strh w14, [x4], #2
    sub x16, x16, #2
46:
    cbz x16, 13f
    ldrb w14, [x15], #1
    strb w14, [x4], #1
13:
    add x0, x9, #1
    b 2b
20:
    sub x0, x4, x6
    b 22f
21:
    mov x0, #-1
22:
