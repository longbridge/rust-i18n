# Legacy scanner: one repne scasb over the whole slice.
# sysv64: rdi = pointer, rsi = length.
# Returns rax = index of the first '%' byte, or usize::MAX when absent.
# Clobbers rcx, rdx, rdi and flags; all are caller-saved.
    test rsi, rsi
    jz 3f
    mov rdx, rdi
    mov rcx, rsi
    mov eax, 0x25
    cld
    repne scasb
    jne 3f
    # A match leaves rdi one byte past the matching byte.
    lea rax, [rdi - 1]
    sub rax, rdx
    ret
3:
    mov rax, -1
    ret
