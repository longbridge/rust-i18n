// NEON scanner: 16-byte blocks reduced with umaxv. A block with a match is
// searched bytewise, then the final partial block is searched bytewise.
// Matches the AArch64 path of benches/support/scan_intrinsics.rs.
// AAPCS64: x0 = pointer, x1 = length.
// Returns x0 = index of the first '%' byte, or usize::MAX when absent.
// Clobbers x2-x4, v0, v1 and flags; all are caller-saved. x18 is untouched.
    movi v1.16b, #0x25
    mov x2, #0
    and x3, x1, #0xfffffffffffffff0
2:
    // x2 = offset of the next block; x3 = end of the last whole block.
    cmp x2, x3
    b.hs 4f
    ldr q0, [x0, x2]
    cmeq v0.16b, v0.16b, v1.16b
    umaxv b0, v0.16b
    umov w4, v0.b[0]
    cbnz w4, 3f
    add x2, x2, #16
    b 2b
3:
    // The block holds a match: search its 16 bytes.
    add x3, x2, #16
    b 5f
4:
    // Search the tail up to the slice length.
    mov x3, x1
5:
    cmp x2, x3
    b.hs 6f
    ldrb w4, [x0, x2]
    cmp w4, #0x25
    b.eq 7f
    add x2, x2, #1
    b 5b
6:
    mov x0, #-1
    ret
7:
    mov x0, x2
    ret
