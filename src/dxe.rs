use core::arch::asm;

mod hob;

mod memory;
use self::memory::{DxeAllocator, PAGE_SIZE};

fn debug_write(bytes: &[u8]) {
    for &byte in bytes {
        unsafe {
            asm!(
                "out dx, al",
                in("dx") 0xE9u16,
                in("al") byte,
                options(nomem, nostack, preserves_flags)
            );
        }
    }
}

fn debug_hex(value: u64) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";

    for shift in (0..16).rev() {
        let digit = ((value >> (shift * 4)) & 0xF) as usize;
        debug_write(&[HEX[digit]]);
    }
}

fn dxe_error(reason: &[u8]) -> ! {
    debug_write(b"DXE VALIDATION FAILED: ");
    debug_write(reason);
    debug_write(b"\r\n");
    halt()
}


pub fn dxe_main(hob_address: u64) -> ! {
    debug_write(b"\r\nFERRIFIRM: ENTERING DXE\r\n");
    debug_write(b"DXE: VALIDATING PEI HANDOFF\r\n");

    let validated = match hob::validate(hob_address) {
        Ok(result) => result,
        Err(reason) => dxe_error(reason),
    };

    let mut allocator = DxeAllocator::new(
        validated.free_start,
        validated.free_end,
    ).unwrap_or_else(|| dxe_error(b"INVALID ALLOCATOR MEMORY RANGE"));

    let initial_free = allocator.remaining_bytes();

    let first_page = allocator.allocate_pages(1)
        .unwrap_or_else(|_| dxe_error(b"FIRST PAGE ALLOCATION FAILED"));

    let four_pages = allocator.allocate_pages(4)
        .unwrap_or_else(|_| dxe_error(b"FOUR-PAGE ALLOCATION FAILED"));

    if four_pages != first_page + PAGE_SIZE {
        dxe_error(b"ALLOCATOR SEQUENCE ERROR");
    }


    if allocator.active_allocations() != 2 {
        dxe_error(b"ALLOCATION TRACKING FAILED");
    }

    allocator.free_pages(four_pages, 4)
        .unwrap_or_else(|_| dxe_error(b"FOUR-PAGE FREE FAILED"));

    if !matches!(
        allocator.free_pages(four_pages, 4),
        Err(memory::FreeError::InvalidAllocation)
    ) {
        dxe_error(b"DOUBLE FREE WAS NOT REJECTED");
    }

    let reused_pages = allocator.allocate_pages(4)
        .unwrap_or_else(|_| dxe_error(b"PAGE REUSE FAILED"));

    if reused_pages != four_pages {
        dxe_error(b"FREED MEMORY NOT REUSED");
    }

    allocator.free_pages(reused_pages, 4)
        .unwrap_or_else(|_| dxe_error(b"REUSED PAGE FREE FAILED"));

    allocator.free_pages(first_page, 1)
        .unwrap_or_else(|_| dxe_error(b"FIRST PAGE FREE FAILED"));

    if allocator.remaining_bytes() != initial_free {
        dxe_error(b"MEMORY RECLAMATION FAILED");
    }

    if allocator.active_allocations() != 0 {
        dxe_error(b"ALLOCATIONS REMAIN ACTIVE");
    }

    debug_write(b"DXE: PHIT VERIFIED\r\n");

    debug_write(b"DXE: HOB ENTRIES=");
    debug_hex(validated.hob_count as u64);
    debug_write(b"\r\n");

    debug_write(b"DXE: RESOURCE DESCRIPTORS VERIFIED=");
    debug_hex(validated.resource_count as u64);
    debug_write(b"\r\n");

    debug_write(b"DXE: MEMORY ALLOCATIONS VERIFIED=");
    debug_hex(validated.allocation_count as u64);
    debug_write(b"\r\n");

    debug_write(b"DXE: PAGE TABLE ALLOCATION VERIFIED\r\n");
    debug_write(b"DXE: BSP STACK ALLOCATION VERIFIED\r\n");

    debug_write(b"DXE: FREE MEMORY START=0x");
    debug_hex(validated.free_start);
    debug_write(b"\r\n");

    debug_write(b"DXE: FREE MEMORY END=0x");
    debug_hex(validated.free_end);
    debug_write(b"\r\n");

    debug_write(b"DXE: HOB LIST VALIDATION PASSED\r\n");
    debug_write(b"FERRIFIRM: DXE INITIALIZATION COMPLETE\r\n");

    debug_write(b"DXE: PAGE ALLOCATOR INITIALIZED\r\n");
    debug_write(b"DXE: ALLOCATION TRACKING PASSED\r\n");
    debug_write(b"DXE: DOUBLE-FREE PROTECTION PASSED\r\n");
    debug_write(b"DXE: PAGE REUSE PASSED\r\n");
    debug_write(b"DXE: MEMORY RECLAMATION PASSED\r\n");


    debug_write(b"DXE: FIRST PAGE ALLOCATION=0x");
    debug_hex(first_page);
    debug_write(b"\r\n");

    debug_write(b"DXE: FOUR-PAGE ALLOCATION=0x");
    debug_hex(four_pages);
    debug_write(b"\r\n");

    debug_write(b"DXE: ALLOCATOR REMAINING BYTES=0x");
    debug_hex(allocator.remaining_bytes());
    debug_write(b"\r\n");

    halt()
}

fn halt() -> ! {
    loop {
        unsafe {
            asm!("cli", "hlt", options(nomem, nostack));
        }
    }
}
