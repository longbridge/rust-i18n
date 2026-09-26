# SSE2 scanner: 16-byte blocks, then a scalar tail.
# Matches the SSE2 path of benches/support/scan_intrinsics.rs.
# sysv64: rdi = pointer, rsi = length.
# Returns rax = index of the first '%' byte, or usize::MAX when absent.
# Clobbers rcx, rdx, xmm0, xmm1 and flags; all are caller-saved.
    mov eax, 0x25252525
    movd xmm1, eax
    pshufd xmm1, xmm1, 0
    xor eax, eax
    mov rcx, rsi
    and rcx, -16
2:
    # rax = offset of the next block; rcx = end of the last whole block.
    cmp rax, rcx
    jae 4f
    movdqu xmm0, xmmword ptr [rdi + rax]
    pcmpeqb xmm0, xmm1
    pmovmskb edx, xmm0
    test edx, edx
    jnz 3f
    add rax, 16
    jmp 2b
3:
    bsf edx, edx
    add rax, rdx
    ret
4:
    # Scalar tail: fewer than 16 bytes remain.
    cmp rax, rsi
    jae 5f
    cmp byte ptr [rdi + rax], 0x25
    je 6f
    inc rax
    jmp 4b
5:
    mov rax, -1
6:
    ret
