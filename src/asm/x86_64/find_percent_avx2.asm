# AVX2 scanner: 32-byte blocks, then a scalar tail. The caller must check
# for AVX2 support first.
# Matches the AVX2 path of benches/support/scan_intrinsics.rs.
# sysv64: rdi = pointer, rsi = length.
# Returns rax = index of the first '%' byte, or usize::MAX when absent.
# Clobbers rcx, rdx, ymm0, ymm1 and flags; all are caller-saved. Upper YMM
# state is cleared with vzeroupper before returning.
    mov eax, 0x25
    vmovd xmm1, eax
    vpbroadcastb ymm1, xmm1
    xor eax, eax
    mov rcx, rsi
    and rcx, -32
2:
    # rax = offset of the next block; rcx = end of the last whole block.
    cmp rax, rcx
    jae 4f
    vmovdqu ymm0, ymmword ptr [rdi + rax]
    vpcmpeqb ymm0, ymm0, ymm1
    vpmovmskb edx, ymm0
    test edx, edx
    jnz 3f
    add rax, 32
    jmp 2b
3:
    vzeroupper
    bsf edx, edx
    add rax, rdx
    ret
4:
    vzeroupper
5:
    # Scalar tail: fewer than 32 bytes remain.
    cmp rax, rsi
    jae 6f
    cmp byte ptr [rdi + rax], 0x25
    je 7f
    inc rax
    jmp 5b
6:
    mov rax, -1
7:
    ret
