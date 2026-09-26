// x86-64 interpolation kernel: the whole body of a naked function.
//
// ABI (System V; declared `extern "sysv64"` so Windows uses it too):
//   rdi  input          readable input bytes
//   rsi  input_len
//   rdx  patterns       descriptors, 32 bytes each: key ptr, key len,
//                       value ptr, value len at offsets 0, 8, 16, 24
//   rcx  pattern_count
//   r8   output         writable bytes, disjoint from every readable source
//   r9   capacity
// Returns rax = bytes written, rdx = input bytes consumed.
//   consumed == input_len: complete; the output is the whole interpolation.
//   consumed <  input_len: the output is full. The unconsumed input starts
//                          at literal text or at a marker percent sign, so
//                          the caller may grow the output and call again.
//   rax == usize::MAX:     rejected (stray, nested or unfinished percent);
//                          the caller discards the output and uses Rust.
// Saves and restores r12-r15; clobbers rax, rcx, rdx, rsi, rdi, r8-r11 and
// xmm0-xmm3. Uses 40 bytes of stack and calls nothing. Reads stay inside
// the input, key and value slices; stores stay inside the output capacity,
// though bytes past the returned length may be overwritten.
//
// Internal registers: rsi input cursor, rdx input end, r15 input start,
// r8/r9 descriptor start/end, rdi output cursor, r10 output end,
// xmm1 splat of percent, xmm3 splat of the closing brace.
    push r12
    push r13
    push r14
    push r15
    push r8
    mov r15, rdi
    lea r10, [r8 + r9]
    shl rcx, 5
    lea r9, [rdx + rcx]
    mov r8, rdx
    lea rdx, [rdi + rsi]
    mov rsi, rdi
    mov rdi, qword ptr [rsp]
    mov eax, 0x25252525
    movd xmm1, eax
    pshufd xmm1, xmm1, 0
    mov eax, 0x7d7d7d7d
    movd xmm3, eax
    pshufd xmm3, xmm3, 0
2:
    mov rcx, rdx
    sub rcx, rsi
    cmp rcx, 16
    jb 40f
    mov rax, r10
    sub rax, rdi
    cmp rax, 16
    jb 90f
    movdqu xmm0, xmmword ptr [rsi]
    movdqu xmmword ptr [rdi], xmm0
    pcmpeqb xmm0, xmm1
    pmovmskb eax, xmm0
    test eax, eax
    jnz 3f
    add rsi, 16
    add rdi, 16
    jmp 2b
3:
    bsf eax, eax
    add rsi, rax
    add rdi, rax
    jmp 50f
40:
    test rcx, rcx
    jz 80f
    lea rax, [r15 + 16]
    cmp rdx, rax
    jb 45f
    movdqu xmm0, xmmword ptr [rdx - 16]
    pcmpeqb xmm0, xmm1
    pmovmskb eax, xmm0
    mov r13, rcx
    neg ecx
    add ecx, 16
    shr eax, cl
    test eax, eax
    jz 42f
    bsf eax, eax
    test eax, eax
    jz 50f
    mov r13, rax
42:
    mov r11, rsi
    lea r12, [rsi + r13 - 1]
    jmp 60f
45:
    mov r11, rsi
46:
    cmp rsi, rdx
    je 47f
    cmp byte ptr [rsi], 37
    je 47f
    inc rsi
    jmp 46b
47:
    mov r13, rsi
    sub r13, r11
    lea r12, [rsi - 1]
    mov rsi, r11
    test r13, r13
    jnz 60f
    cmp rsi, rdx
    je 80f
50:
    lea r11, [rsi + 2]
    cmp r11, rdx
    ja 95f
    cmp byte ptr [rsi + 1], 123
    jne 95f
    mov rcx, rdx
    sub rcx, r11
    cmp rcx, 16
    jb 53f
    movdqu xmm0, xmmword ptr [r11]
    movdqa xmm2, xmm0
    pcmpeqb xmm0, xmm3
    pcmpeqb xmm2, xmm1
    por xmm0, xmm2
    pmovmskb eax, xmm0
    test eax, eax
    jz 55f
    bsf eax, eax
    jmp 57f
53:
    test rcx, rcx
    jz 95f
    lea rax, [r15 + 16]
    cmp rdx, rax
    jb 56f
    movdqu xmm0, xmmword ptr [rdx - 16]
    movdqa xmm2, xmm0
    pcmpeqb xmm0, xmm3
    pcmpeqb xmm2, xmm1
    por xmm0, xmm2
    pmovmskb eax, xmm0
    neg ecx
    add ecx, 16
    shr eax, cl
    test eax, eax
    jz 95f
    bsf eax, eax
    jmp 57f
55:
    lea r12, [r11 + 16]
    jmp 58f
56:
    mov r12, r11
58:
    cmp r12, rdx
    jae 95f
    movzx eax, byte ptr [r12]
    cmp al, 125
    je 59f
    cmp al, 37
    je 95f
    inc r12
    jmp 58b
59:
    mov rax, r12
    sub rax, r11
    jmp 61f
57:
    cmp byte ptr [r11 + rax], 125
    jne 95f
61:
    mov r13, r8
    cmp rax, 8
    jae 70f
    cmp rax, 4
    jae 66f
    cmp rax, 2
    jae 64f
    test rax, rax
    jz 62f
    movzx r14d, byte ptr [r11]
63:
    cmp r13, r9
    jae 78f
    cmp qword ptr [r13 + 8], rax
    jne 5f
    mov rcx, qword ptr [r13]
    cmp r14b, byte ptr [rcx]
    je 77f
5:
    add r13, 32
    jmp 63b
62:
    cmp r13, r9
    jae 78f
    cmp qword ptr [r13 + 8], 0
    je 77f
    add r13, 32
    jmp 62b
64:
    movzx r14d, word ptr [r11]
    movzx r12d, word ptr [r11 + rax - 2]
65:
    cmp r13, r9
    jae 78f
    cmp qword ptr [r13 + 8], rax
    jne 6f
    mov rcx, qword ptr [r13]
    cmp r14w, word ptr [rcx]
    jne 6f
    cmp r12w, word ptr [rcx + rax - 2]
    je 77f
6:
    add r13, 32
    jmp 65b
66:
    mov r14d, dword ptr [r11]
    mov r12d, dword ptr [r11 + rax - 4]
67:
    cmp r13, r9
    jae 78f
    cmp qword ptr [r13 + 8], rax
    jne 7f
    mov rcx, qword ptr [r13]
    cmp r14d, dword ptr [rcx]
    jne 7f
    cmp r12d, dword ptr [rcx + rax - 4]
    je 77f
7:
    add r13, 32
    jmp 67b
70:
    cmp rax, 16
    ja 73f
    mov r14, qword ptr [r11]
    mov r12, qword ptr [r11 + rax - 8]
71:
    cmp r13, r9
    jae 78f
    cmp qword ptr [r13 + 8], rax
    jne 8f
    mov rcx, qword ptr [r13]
    cmp r14, qword ptr [rcx]
    jne 8f
    cmp r12, qword ptr [rcx + rax - 8]
    je 77f
8:
    add r13, 32
    jmp 71b
73:
    cmp r13, r9
    jae 78f
    cmp qword ptr [r13 + 8], rax
    jne 9f
    mov rcx, qword ptr [r13]
    xor esi, esi
74:
    mov r14, qword ptr [r11 + rsi]
    cmp r14, qword ptr [rcx + rsi]
    jne 9f
    add rsi, 8
    lea r14, [rsi + 8]
    cmp r14, rax
    jb 74b
    mov r14, qword ptr [r11 + rax - 8]
    cmp r14, qword ptr [rcx + rax - 8]
    je 77f
9:
    add r13, 32
    jmp 73b
77:
    mov r12, qword ptr [r13 + 8]
    add r12, r11
    lea rsi, [r11 - 2]
    mov r11, qword ptr [r13 + 16]
    mov r13, qword ptr [r13 + 24]
    jmp 60f
78:
    lea r12, [r11 + rax]
    lea rsi, [r11 - 2]
    mov r11, rsi
    lea r13, [rax + 3]
60:
    mov rax, r10
    sub rax, rdi
    cmp r13, rax
    ja 90f
    cmp r13, 16
    ja 26f
    cmp r13, 8
    jae 25f
    cmp r13, 4
    jae 24f
    test r13, r13
    jz 69f
    movzx eax, byte ptr [r11]
    movzx ecx, byte ptr [r11 + r13 - 1]
    mov byte ptr [rdi], al
    mov byte ptr [rdi + r13 - 1], cl
    mov rax, r13
    shr rax, 1
    movzx ecx, byte ptr [r11 + rax]
    mov byte ptr [rdi + rax], cl
    jmp 69f
24:
    mov eax, dword ptr [r11]
    mov ecx, dword ptr [r11 + r13 - 4]
    mov dword ptr [rdi], eax
    mov dword ptr [rdi + r13 - 4], ecx
    jmp 69f
25:
    mov rax, qword ptr [r11]
    mov rcx, qword ptr [r11 + r13 - 8]
    mov qword ptr [rdi], rax
    mov qword ptr [rdi + r13 - 8], rcx
    jmp 69f
26:
    movdqu xmm2, xmmword ptr [r11 + r13 - 16]
    cmp r13, 32
    ja 27f
    movdqu xmm0, xmmword ptr [r11]
    movdqu xmmword ptr [rdi], xmm0
    movdqu xmmword ptr [rdi + r13 - 16], xmm2
    jmp 69f
27:
    xor eax, eax
28:
    movdqu xmm0, xmmword ptr [r11 + rax]
    movdqu xmmword ptr [rdi + rax], xmm0
    add rax, 16
    lea rcx, [rax + 16]
    cmp rcx, r13
    jb 28b
    movdqu xmmword ptr [rdi + r13 - 16], xmm2
69:
    add rdi, r13
    lea rsi, [r12 + 1]
    jmp 2b
80:
90:
    mov rax, rdi
    sub rax, qword ptr [rsp]
    mov rdx, rsi
    sub rdx, r15
    jmp 99f
95:
    mov rax, -1
    xor edx, edx
99:
    add rsp, 8
    pop r15
    pop r14
    pop r13
    pop r12
    ret
