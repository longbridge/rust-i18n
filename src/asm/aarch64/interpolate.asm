// AArch64 interpolation kernel: the whole body of a naked function.
//
// ABI (AAPCS64, declared `extern "C"`):
//   x0  input          readable input bytes
//   x1  input_len
//   x2  patterns       descriptors, 32 bytes each: key ptr, key len,
//                      value ptr, value len at offsets 0, 8, 16, 24
//   x3  pattern_count
//   x4  output         writable bytes, disjoint from every readable source
//   x5  capacity
// Returns x0 = bytes written, x1 = input bytes consumed.
//   consumed == input_len: complete; the output is the whole interpolation.
//   consumed <  input_len: the output is full. The unconsumed input starts
//                          at literal text or at a percent sign, so the
//                          caller may grow the output and call again.
//   x0 == usize::MAX:      rejected: a percent sign inside a marker, or a
//                          stray percent sign followed by an opening brace
//                          before the next percent sign. The caller
//                          discards the output and uses the legacy parser.
// Other input follows the legacy parser: a stray percent sign and an
// unfinished final marker (percent, opening brace, and no closing brace or
// percent sign before the end) are literal text.
// Clobbers x0-x17, v0-v5 and NZCV. Saves nothing: x18 and x19-x28, v8-v15
// are never touched. Uses no stack and calls nothing. Reads stay inside the
// input, key and value slices; stores stay inside the output capacity,
// though bytes past the returned length may be overwritten.
//
// Internal registers: x0 input cursor, x1 input end, d4 input start,
// x2/x3 descriptor array and count, x4 output cursor, x5 output end,
// x6 output start, x7-x17 scratch, v1 splat of percent, v3 splat of the
// closing brace, v5 splat of the opening brace, v0/v2 data and comparison
// scratch.
// Every load checks the remaining source length and every store checks the
// remaining output capacity first. Vector stores of literal text may write
// past the advanced cursor, but never past x5; those bytes are rewritten
// or left outside the returned initialized length. Literal text is copied a
// chunk or byte at a time and each marker is copied whole, so running out
// of output stops at literal text or at a percent sign.
    fmov d4, x0
    add x1, x0, x1
    add x5, x4, x5
    mov x6, x4
    movi v1.16b, #37
    movi v3.16b, #125
    movi v5.16b, #123
2:
    // Literal text. Find the next '%' 16, then 8, then 1 byte at a time.
    sub x17, x1, x0
    cbz x17, 90f
    sub x16, x5, x4
    cmp x17, #16
    b.lo 34f
    cmp x16, #16
    b.lo 34f
    ldr q0, [x0]
    cmeq v2.16b, v0.16b, v1.16b
    shrn v2.8b, v2.8h, #4
    fmov x14, d2
    cbnz x14, 31f
    str q0, [x4], #16
    add x0, x0, #16
    b 2b
31:
    // Four mask bits per byte: store the whole chunk, then advance both
    // cursors to the first '%'.
    str q0, [x4]
    rbit x14, x14
    clz x14, x14
    lsr x14, x14, #2
    add x4, x4, x14
    add x0, x0, x14
    b 3f
34:
    cmp x17, #8
    b.lo 36f
    cmp x16, #8
    b.lo 36f
    ldr d0, [x0]
    cmeq v2.8b, v0.8b, v1.8b
    fmov x14, d2
    cbnz x14, 35f
    str d0, [x4], #8
    add x0, x0, #8
    b 2b
35:
    // Eight mask bits per byte.
    str d0, [x4]
    rbit x14, x14
    clz x14, x14
    lsr x14, x14, #3
    add x4, x4, x14
    add x0, x0, x14
    b 3f
36:
    ldrb w14, [x0]
    cmp w14, #37
    b.eq 3f
    cbz x16, 90f
    strb w14, [x4], #1
    add x0, x0, #1
    b 2b
3:
    // x0 points at '%'. Without an open brace it is a stray percent sign.
    // Scan the key to the close brace: a nested '%' rejects, and reaching
    // the end first makes the unfinished marker literal text.
    add x8, x0, #1
    cmp x8, x1
    b.hs 60f
    ldrb w14, [x8]
    cmp w14, #123
    b.ne 60f
    add x8, x8, #1
    mov x9, x8
42:
    sub x17, x1, x9
    cmp x17, #16
    b.lo 4f
    ldr q0, [x9]
    cmeq v2.16b, v0.16b, v1.16b
    cmeq v0.16b, v0.16b, v3.16b
    orr v2.16b, v2.16b, v0.16b
    shrn v2.8b, v2.8h, #4
    fmov x14, d2
    cbnz x14, 43f
    add x9, x9, #16
    b 42b
43:
    rbit x14, x14
    clz x14, x14
    add x9, x9, x14, lsr #2
    b 46f
4:
    sub x17, x1, x9
    cmp x17, #8
    b.lo 45f
    ldr d0, [x9]
    cmeq v2.8b, v0.8b, v1.8b
    cmeq v0.8b, v0.8b, v3.8b
    orr v2.8b, v2.8b, v0.8b
    fmov x14, d2
    cbnz x14, 44f
    add x9, x9, #8
    b 4b
44:
    rbit x14, x14
    clz x14, x14
    add x9, x9, x14, lsr #3
46:
    ldrb w14, [x9]
    cmp w14, #37
    b.eq 95f
    b 5f
45:
    cmp x9, x1
    b.hs 68f
    ldrb w14, [x9]
    cmp w14, #125
    b.eq 5f
    cmp w14, #37
    b.eq 95f
    add x9, x9, #1
    b 45b
5:
    // Key is [x8, x9). The first descriptor with an equal key wins.
    sub x12, x9, x8
    mov x10, x2
    mov x11, x3
6:
    cbz x11, 10f
    ldp x15, x16, [x10]
    cmp x16, x12
    b.ne 8f
    cbz x12, 9f
    cmp x12, #8
    b.lo 40f
    // Length >= 8: 8-byte chunks, then an overlapping final chunk.
    sub x7, x12, #8
    mov x13, #0
7:
    cmp x13, x7
    b.hs 71f
    ldr x14, [x8, x13]
    ldr x17, [x15, x13]
    cmp x14, x17
    b.ne 8f
    add x13, x13, #8
    b 7b
71:
    ldr x14, [x8, x7]
    ldr x17, [x15, x7]
    cmp x14, x17
    b.ne 8f
    b 9f
40:
    // Length 4..=7: two overlapping 4-byte chunks.
    cmp x12, #4
    b.lo 41f
    sub x7, x12, #4
    ldr w14, [x8]
    ldr w17, [x15]
    cmp w14, w17
    b.ne 8f
    ldr w14, [x8, x7]
    ldr w17, [x15, x7]
    cmp w14, w17
    b.ne 8f
    b 9f
41:
    // Length 1..=3: first byte, then the final two bytes when present.
    ldrb w14, [x8]
    ldrb w17, [x15]
    cmp w14, w17
    b.ne 8f
    cmp x12, #1
    b.eq 9f
    sub x7, x12, #2
    ldrh w14, [x8, x7]
    ldrh w17, [x15, x7]
    cmp w14, w17
    b.ne 8f
    b 9f
8:
    add x10, x10, #32
    sub x11, x11, #1
    b 6b
9:
    ldp x15, x16, [x10, #16]
    b 11f
10:
    // Unknown key: copy the complete marker through its close brace.
    sub x15, x8, #2
    sub x16, x9, x15
    add x16, x16, #1
11:
    // Copy x16 bytes from x15 after checking output capacity. When it does
    // not fit, x0 still points at the percent sign.
    sub x17, x5, x4
    cmp x16, x17
    b.hi 90f
    add x13, x15, x16
    add x7, x4, x16
    cmp x16, #16
    b.lo 50f
    // Length >= 16: 32/16-byte chunks, then an overlapping final 16 bytes.
12:
    cmp x16, #32
    b.ls 14f
    ldp q0, q2, [x15], #32
    stp q0, q2, [x4], #32
    sub x16, x16, #32
    b 12b
14:
    cmp x16, #16
    b.ls 15f
    ldr q0, [x15], #16
    str q0, [x4], #16
15:
    ldur q0, [x13, #-16]
    stur q0, [x7, #-16]
    b 13f
50:
    // Length 8..=15: two overlapping 8-byte chunks.
    cmp x16, #8
    b.lo 51f
    ldr x14, [x15]
    ldur x17, [x13, #-8]
    str x14, [x4]
    stur x17, [x7, #-8]
    b 13f
51:
    // Length 4..=7: two overlapping 4-byte chunks.
    cmp x16, #4
    b.lo 52f
    ldr w14, [x15]
    ldur w17, [x13, #-4]
    str w14, [x4]
    stur w17, [x7, #-4]
    b 13f
52:
    // Length 1..=3: bytes 0, len / 2 and len - 1 cover every byte.
    cbz x16, 13f
    lsr x12, x16, #1
    ldrb w14, [x15]
    ldrb w17, [x15, x12]
    ldurb w11, [x13, #-1]
    strb w14, [x4]
    strb w17, [x4, x12]
    sturb w11, [x7, #-1]
13:
    mov x4, x7
    add x0, x9, #1
    b 2b
60:
    // Stray '%' at x0. It is literal only when no open brace comes before
    // the next '%'; otherwise the legacy parser opens a marker at that
    // brace, so reject. The literal run up to the next '%' or the end is
    // copied as one block, and a close brace inside it does not end it.
    add x9, x0, #1
62:
    sub x17, x1, x9
    cmp x17, #16
    b.lo 64f
    ldr q0, [x9]
    cmeq v2.16b, v0.16b, v1.16b
    cmeq v0.16b, v0.16b, v5.16b
    orr v2.16b, v2.16b, v0.16b
    shrn v2.8b, v2.8h, #4
    fmov x14, d2
    cbnz x14, 63f
    add x9, x9, #16
    b 62b
63:
    rbit x14, x14
    clz x14, x14
    add x9, x9, x14, lsr #2
    ldrb w14, [x9]
    cmp w14, #123
    b.eq 95f
    b 69f
64:
    cmp x9, x1
    b.hs 69f
    ldrb w14, [x9]
    cmp w14, #37
    b.eq 69f
    cmp w14, #123
    b.eq 95f
    add x9, x9, #1
    b 64b
69:
    // Copy [x0, x9) and resume at x9. The copy continues after x9 - 1.
    mov x15, x0
    sub x16, x9, x0
    sub x9, x9, #1
    b 11b
68:
    // Unfinished final marker: copy [x0, x1) literally and complete.
    mov x15, x0
    sub x16, x1, x0
    sub x9, x1, #1
    b 11b
90:
    // Complete or output full.
    fmov x9, d4
    sub x1, x0, x9
    sub x0, x4, x6
    ret
95:
    // Rejected.
    mov x0, #-1
    mov x1, #0
    ret
