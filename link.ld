ENTRY(_start)

SECTIONS
{
    . = 0xFFF00000;

    /*
     * The assembly entry MUST be the first executable code.
     * Collect Rust large-model sections afterwards.
     */
    .text :
    {
        KEEP(*(.text.boot))
        *(.text .text.*)
        *(.ltext .ltext.*)
    }

    .rodata :
    {
        *(.rodata .rodata.*)
        *(.lrodata .lrodata.*)
    }

    .data :
    {
        *(.got .got.*)
        *(.igot.plt)
        *(.data .data.*)
        *(.ldata .ldata.*)
    }

    .bss (NOLOAD) :
    {
        *(.bss .bss.*)
        *(.lbss .lbss.*)
        *(COMMON)
    }

    ASSERT(_start == 0xFFF00000,
           "_start MUST be at firmware entry address")

    ASSERT(. <= 0xFFFF0000,
           "Firmware sections overlap boot stub")

    . = 0xFFFF0000;

    .boot_stub :
    {
        KEEP(*(.boot_stub))
    }

    ASSERT(SIZEOF(.boot_stub) < 0xFFF0,
           "Boot stub overlaps reset vector")

    . = 0xFFFFFFF0;

    .reset :
    {
        KEEP(*(.reset))
    }

    ASSERT(ADDR(.reset) == 0xFFFFFFF0,
           "Reset vector is at wrong address")

    ASSERT(SIZEOF(.reset) == 16,
           "Reset vector must be exactly 16 bytes")

    /DISCARD/ :
    {
        *(.eh_frame*)
        *(.comment*)
        *(.note*)
    }
}
