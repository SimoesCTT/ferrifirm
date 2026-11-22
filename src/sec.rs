use core::arch::global_asm;

global_asm!(
    r#"

.section .boot_stub,"ax"
.code16
.global _boot_stub

_boot_stub:
    cli
    cld

    /* Establish a known real-mode stack and data segments. */
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7C00

    /* S = reset vector successfully reached the boot stub. */
    mov al, 'S'
    out 0xE9, al

    /* GDT descriptor is at CS:0100 (physical FFFF0100). */
    .byte 0x66
    lgdt cs:[0x0100]

    /* Enable protected mode. */
    mov eax, cr0
    or eax, 1
    mov cr0, eax

    /* 16-bit code, but load a 32-bit protected-mode EIP. */
    .byte 0x66, 0xEA
    .long 0xFFFF0300
    .word 0x08

    /*
     * GDT descriptor at section offset 0x100.
     * GDT itself begins at section offset 0x110.
     */
    .org 0x100
gdt_pointer:
    .word 0x001F
    .long 0xFFFF0110

    .org 0x110
gdt:
    .quad 0x0000000000000000
    .quad 0x00CF9A000000FFFF    /* 0x08: 32-bit code */
    .quad 0x00CF92000000FFFF    /* 0x10: 32-bit data */
    .quad 0x00AF9A000000FFFF    /* 0x18: 64-bit code */

    /*
     * Protected-mode entry at physical FFFF0300.
     */
    .org 0x300
.code32
protected_entry:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov esp, 0x7C00

    /*
     * Clear six 4 KiB pages of page-table memory:
     * PML4  = 0x100000
     * PDPT  = 0x101000
     * PD[0] = 0x102000
     * PD[1] = 0x103000
     * PD[2] = 0x104000
     * PD[3] = 0x105000
     */
    mov edi, 0x100000
    xor eax, eax
    mov ecx, 6144
    rep stosd

    /* PML4[0] -> PDPT */
    mov dword ptr [0x100000], 0x101003

    /* PDPT entries -> four page directories */
    mov dword ptr [0x101000], 0x102003
    mov dword ptr [0x101008], 0x103003
    mov dword ptr [0x101010], 0x104003
    mov dword ptr [0x101018], 0x105003

    /*
     * Populate 2048 2 MiB page-directory entries.
     * This identity-maps physical addresses 0 through 4 GiB.
     */
    mov edi, 0x102000
    mov eax, 0x00000083
    mov ecx, 2048

map_2m_pages:
    mov dword ptr [edi], eax
    mov dword ptr [edi + 4], 0
    add edi, 8
    add eax, 0x00200000
    loop map_2m_pages

    /* Enable PAE (CR4.PAE, bit 5). */
    mov eax, cr4
    or eax, 0x20
    mov cr4, eax

    /* Load PML4 physical address. */
    mov eax, 0x100000
    mov cr3, eax

    /* Enable EFER.LME (Long Mode Enable). */
    mov ecx, 0xC0000080
    rdmsr
    or eax, 0x00000100
    wrmsr

    /* Enable paging. CR0.PG + CR0.PE activates long mode. */
    mov eax, cr0
    or eax, 0x80000000
    mov cr0, eax

    /*
     * Far jump into the 64-bit code segment.
     * Identity mapping makes FFF00000 executable.
     */
    .byte 0xEA
    .long 0xFFF00000
    .word 0x18

"#
);

global_asm!(
    r#"

.section .text.boot,"ax"
.code64
.global _start

_start:
    cli
    cld

    /* Establish a dedicated, 16-byte-aligned firmware stack. */
    mov rsp, 0x120000
    and rsp, -16
    xor rbp, rbp

    /* P = successfully executing 64-bit firmware code. */
    mov dx, 0xE9
    mov al, 'P'
    out dx, al

    /* Enter the Rust SEC implementation using the x86-64 ABI. */
    call sec_main

halt64:
    cli
    hlt
    jmp halt64

"#
);

global_asm!(
    r#"

.section .reset,"ax"
.code16
.global _reset_vector

_reset_vector:
    cli
    cld

    mov al, 'R'
    out 0xE9, al

    /* Near jump: preserve reset-time CS hidden base. */
    .byte 0xE9
    .word 0x0007

    .fill 7, 1, 0x90

"#
);
