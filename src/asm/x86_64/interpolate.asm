    mov eax, 0x25252525
    movd xmm1, eax
    pshufd xmm1, xmm1, 0
2:
    cmp rsi, rdx
    je 20f
    mov rcx, rdx
    sub rcx, rsi
    cmp rcx, 16
    jb 34f
    mov rcx, r10
    sub rcx, rdi
    cmp rcx, 16
    jb 34f
    movdqu xmm0, xmmword ptr [rsi]
    movdqa xmm2, xmm0
    pcmpeqb xmm0, xmm1
    pmovmskb eax, xmm0
    test eax, eax
    je 35f
    bsf eax, eax
    test eax, eax
    je 34f
    mov r11, rsi
    lea r12, [rsi + rax - 1]
    mov r13, rax
    jmp 24f
35:
    movdqu xmmword ptr [rdi], xmm2
    add rsi, 16
    add rdi, 16
    jmp 2b
34:
    movzx eax, byte ptr [rsi]
    cmp al, 37
    je 3f
    cmp rdi, r10
    jae 21f
    mov byte ptr [rdi], al
    inc rdi
    inc rsi
    jmp 2b
3:
    lea rcx, [rsi + 1]
    cmp rcx, rdx
    jae 21f
    cmp byte ptr [rcx], 123
    jne 21f
    inc rcx
    mov r12, rcx
4:
    cmp r12, rdx
    jae 21f
    movzx eax, byte ptr [r12]
    cmp al, 125
    je 5f
    cmp al, 37
    je 21f
    inc r12
    jmp 4b
5:
    mov r13, r12
    sub r13, rcx
    xor r14d, r14d
6:
    cmp r14, r9
    jae 23f
    mov r15, r14
    shl r15, 5
    add r15, r8
    cmp qword ptr [r15 + 8], r13
    jne 8f
    mov r11, qword ptr [r15]
    xor eax, eax
7:
    mov rsi, r13
    sub rsi, rax
    cmp rsi, 8
    jb 27f
    mov rsi, qword ptr [rcx + rax]
    cmp rsi, qword ptr [r11 + rax]
    jne 8f
    add rax, 8
    jmp 7b
27:
    cmp rsi, 4
    jb 28f
    mov esi, dword ptr [rcx + rax]
    cmp esi, dword ptr [r11 + rax]
    jne 8f
    add rax, 4
28:
    mov rsi, r13
    sub rsi, rax
    cmp rsi, 2
    jb 29f
    mov si, word ptr [rcx + rax]
    cmp si, word ptr [r11 + rax]
    jne 8f
    add rax, 2
29:
    cmp rax, r13
    je 9f
    mov sil, byte ptr [rcx + rax]
    cmp sil, byte ptr [r11 + rax]
    jne 8f
    jmp 9f
8:
    inc r14
    jmp 6b
9:
    mov r11, qword ptr [r15 + 16]
    mov r13, qword ptr [r15 + 24]
    jmp 24f
23:
    lea r11, [rcx - 2]
    mov r13, r12
    sub r13, r11
    inc r13
24:
    mov rax, r10
    sub rax, rdi
    cmp r13, rax
    ja 21f
25:
    cmp r13, 8
    jb 30f
    mov rax, qword ptr [r11]
    mov qword ptr [rdi], rax
    add r11, 8
    add rdi, 8
    sub r13, 8
    jmp 25b
30:
    cmp r13, 4
    jb 31f
    mov eax, dword ptr [r11]
    mov dword ptr [rdi], eax
    add r11, 4
    add rdi, 4
    sub r13, 4
31:
    cmp r13, 2
    jb 32f
    mov ax, word ptr [r11]
    mov word ptr [rdi], ax
    add r11, 2
    add rdi, 2
    sub r13, 2
32:
    test r13, r13
    je 26f
    mov al, byte ptr [r11]
    mov byte ptr [rdi], al
    inc rdi
26:
    lea rsi, [r12 + 1]
    jmp 2b
20:
    mov rax, rdi
    jmp 22f
21:
    mov rax, -1
22:
