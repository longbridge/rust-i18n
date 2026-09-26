2:
    cmp rsi, rdx
    je 20f
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
    cmp rax, r13
    je 9f
    movzx esi, byte ptr [rcx + rax]
    cmp sil, byte ptr [r11 + rax]
    jne 8f
    inc rax
    jmp 7b
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
    test r13, r13
    je 26f
    mov al, byte ptr [r11]
    mov byte ptr [rdi], al
    inc r11
    inc rdi
    dec r13
    jmp 25b
26:
    lea rsi, [r12 + 1]
    jmp 2b
20:
    mov rax, rdi
    jmp 22f
21:
    mov rax, -1
22:
