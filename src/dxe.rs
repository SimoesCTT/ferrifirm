use core::arch::asm;
use core::ptr;

mod memory;
use self::memory::{DxeAllocator, PAGE_SIZE};

const HOB_TYPE_HANDOFF: u16 = 0x0001;
const HOB_TYPE_MEMORY_ALLOCATION: u16 = 0x0002;
const HOB_TYPE_RESOURCE_DESCRIPTOR: u16 = 0x0003;
const HOB_TYPE_END: u16 = 0xFFFF;

const HOB_VERSION: u32 = 0x000A;
const HOB_CAPACITY: u64 = 0x4000;

const RESOURCE_SYSTEM_MEMORY: u32 = 0;
const RESOURCE_MEMORY_RESERVED: u32 = 5;

const RESOURCE_PRESENT: u32 = 0x1;
const RESOURCE_INITIALIZED: u32 = 0x2;

const MEMORY_TYPE_BOOT_SERVICES_DATA: u32 = 4;

const PAGE_TABLE_BASE: u64 = 0x0010_0000;
const PAGE_TABLE_LENGTH: u64 = 0x0000_6000;

const BSP_STACK_BASE: u64 = 0x0011_0000;
const BSP_STACK_LENGTH: u64 = 0x0001_0000;

const MAX_RESOURCE_DESCRIPTORS: usize = 129;

#[repr(C)]
#[derive(Clone, Copy)]
struct HobHeader {
    hob_type: u16,
    hob_length: u16,
    reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

impl PartialEq for Guid {
    fn eq(&self, other: &Self) -> bool {
        self.data1 == other.data1
            && self.data2 == other.data2
            && self.data3 == other.data3
            && self.data4 == other.data4
    }
}

impl Eq for Guid {}

const FERRIFIRM_PAGE_TABLE_GUID: Guid = Guid {
    data1: 0x7d9f2b31,
    data2: 0x6a4c,
    data3: 0x4f18,
    data4: [0x9b, 0x72, 0x3e, 0x65, 0xa1, 0xc8, 0x44, 0x20],
};

const BSP_STACK_GUID: Guid = Guid {
    data1: 0x4ed4bf27,
    data2: 0x4092,
    data3: 0x42e9,
    data4: [0x80, 0x7d, 0x52, 0x7b, 0x1d, 0x00, 0xc9, 0xbd],
};

#[repr(C)]
#[derive(Clone, Copy)]
struct HandoffInfoTable {
    header: HobHeader,
    version: u32,
    boot_mode: u32,
    memory_top: u64,
    memory_bottom: u64,
    free_memory_top: u64,
    free_memory_bottom: u64,
    end_of_hob_list: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ResourceDescriptorHob {
    header: HobHeader,
    owner: [u8; 16],
    resource_type: u32,
    resource_attributes: u32,
    physical_start: u64,
    resource_length: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MemoryAllocationHeader {
    name: Guid,
    memory_base_address: u64,
    memory_length: u64,
    memory_type: u32,
    reserved: [u8; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MemoryAllocationHob {
    header: HobHeader,
    allocation: MemoryAllocationHeader,
}

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

    if hob_address == 0 || hob_address & 7 != 0 {
        dxe_error(b"INVALID HOB ADDRESS");
    }

    if core::mem::size_of::<HobHeader>() != 8
        || core::mem::size_of::<HandoffInfoTable>() != 56
        || core::mem::size_of::<ResourceDescriptorHob>() != 48
        || core::mem::size_of::<MemoryAllocationHob>() != 48
        || core::mem::size_of::<Guid>() != 16
    {
        dxe_error(b"STRUCTURE SIZE MISMATCH");
    }

    let phit = unsafe {
        ptr::read_volatile(hob_address as *const HandoffInfoTable)
    };

    if phit.header.hob_type != HOB_TYPE_HANDOFF
        || phit.header.hob_length as usize
            != core::mem::size_of::<HandoffInfoTable>()
        || phit.header.reserved != 0
        || phit.version != HOB_VERSION
    {
        dxe_error(b"INVALID PHIT");
    }

    if phit.memory_bottom >= phit.memory_top
        || phit.free_memory_bottom >= phit.free_memory_top
        || phit.free_memory_bottom < phit.memory_bottom
        || phit.free_memory_top > phit.memory_top
    {
        dxe_error(b"INVALID PHIT MEMORY RANGE");
    }

    if (phit.memory_bottom & 0xFFF) != 0
        || (phit.memory_top & 0xFFF) != 0
        || (phit.free_memory_bottom & 0xFFF) != 0
        || (phit.free_memory_top & 0xFFF) != 0
    {
        dxe_error(b"UNALIGNED PHIT MEMORY RANGE");
    }

    let minimum_end = match hob_address.checked_add(56) {
        Some(value) => value,
        None => dxe_error(b"HOB ADDRESS OVERFLOW"),
    };

    let maximum_end = match hob_address.checked_add(HOB_CAPACITY - 8) {
        Some(value) => value,
        None => dxe_error(b"HOB CAPACITY OVERFLOW"),
    };

    if phit.end_of_hob_list < minimum_end
        || phit.end_of_hob_list > maximum_end
        || phit.end_of_hob_list & 7 != 0
    {
        dxe_error(b"INVALID HOB END ADDRESS");
    }

    let mut cursor = minimum_end;

    let mut hob_count = 0usize;
    let mut resource_count = 0usize;
    let mut allocation_count = 0usize;

    let mut ranges = [(0u64, 0u64, 0u32); MAX_RESOURCE_DESCRIPTORS];
    let mut allocation_ranges = [(0u64, 0u64); 2];

    let mut page_tables_found = false;
    let mut stack_found = false;

    while cursor < phit.end_of_hob_list {
        let header = unsafe {
            ptr::read_volatile(cursor as *const HobHeader)
        };

        let length = header.hob_length as u64;

        if header.reserved != 0 || length < 8 || length & 7 != 0 {
            dxe_error(b"INVALID HOB HEADER");
        }

        let next = match cursor.checked_add(length) {
            Some(value) => value,
            None => dxe_error(b"HOB LENGTH OVERFLOW"),
        };

        if next > phit.end_of_hob_list {
            dxe_error(b"HOB EXCEEDS END ADDRESS");
        }

        match header.hob_type {
            HOB_TYPE_HANDOFF | HOB_TYPE_END => {
                dxe_error(b"UNEXPECTED HOB TYPE");
            }

            HOB_TYPE_RESOURCE_DESCRIPTOR => {
                if length != 48 {
                    dxe_error(b"INVALID RESOURCE HOB LENGTH");
                }

                let resource = unsafe {
                    ptr::read_volatile(
                        cursor as *const ResourceDescriptorHob
                    )
                };

                if resource.resource_type != RESOURCE_SYSTEM_MEMORY
                    && resource.resource_type != RESOURCE_MEMORY_RESERVED
                {
                    dxe_error(b"UNSUPPORTED RESOURCE TYPE");
                }

                if resource.resource_attributes & RESOURCE_PRESENT == 0 {
                    dxe_error(b"RESOURCE NOT PRESENT");
                }

                if resource.resource_type == RESOURCE_SYSTEM_MEMORY
                    && resource.resource_attributes & RESOURCE_INITIALIZED == 0
                {
                    dxe_error(b"SYSTEM MEMORY NOT INITIALIZED");
                }

                if resource.resource_length == 0
                    || (resource.physical_start & 0xFFF) != 0
                    || (resource.resource_length & 0xFFF) != 0
                {
                    dxe_error(b"INVALID RESOURCE RANGE");
                }

                let range_end = match resource.physical_start
                    .checked_add(resource.resource_length)
                {
                    Some(value) => value,
                    None => dxe_error(b"RESOURCE RANGE OVERFLOW"),
                };

                if resource_count >= ranges.len() {
                    dxe_error(b"TOO MANY RESOURCE DESCRIPTORS");
                }

                for &(previous_start, previous_end, _) in
                    &ranges[..resource_count]
                {
                    if resource.physical_start < previous_end
                        && previous_start < range_end
                    {
                        dxe_error(b"OVERLAPPING RESOURCE RANGES");
                    }
                }

                ranges[resource_count] = (
                    resource.physical_start,
                    range_end,
                    resource.resource_type,
                );

                resource_count += 1;
            }

            HOB_TYPE_MEMORY_ALLOCATION => {
                if length != 48 {
                    dxe_error(b"INVALID ALLOCATION HOB LENGTH");
                }

                let allocation_hob = unsafe {
                    ptr::read_volatile(
                        cursor as *const MemoryAllocationHob
                    )
                };

                let allocation = allocation_hob.allocation;

                if allocation.reserved != [0; 4] {
                    dxe_error(b"ALLOCATION RESERVED FIELD NOT ZERO");
                }

                if allocation.memory_type != MEMORY_TYPE_BOOT_SERVICES_DATA {
                    dxe_error(b"INCORRECT ALLOCATION MEMORY TYPE");
                }

                if allocation.memory_length == 0
                    || (allocation.memory_base_address & 0xFFF) != 0
                    || (allocation.memory_length & 0xFFF) != 0
                {
                    dxe_error(b"INVALID ALLOCATION RANGE");
                }

                let allocation_end = match allocation.memory_base_address
                    .checked_add(allocation.memory_length)
                {
                    Some(value) => value,
                    None => dxe_error(b"ALLOCATION RANGE OVERFLOW"),
                };

                let is_page_tables =
                    allocation.name == FERRIFIRM_PAGE_TABLE_GUID;
                let is_stack = allocation.name == BSP_STACK_GUID;

                if !is_page_tables && !is_stack {
                    dxe_error(b"UNKNOWN ALLOCATION GUID");
                }

                if is_page_tables {
                    if page_tables_found {
                        dxe_error(b"DUPLICATE PAGE TABLE ALLOCATION");
                    }

                    if allocation.memory_base_address != PAGE_TABLE_BASE
                        || allocation.memory_length != PAGE_TABLE_LENGTH
                    {
                        dxe_error(b"INCORRECT PAGE TABLE RANGE");
                    }

                    page_tables_found = true;
                }

                if is_stack {
                    if stack_found {
                        dxe_error(b"DUPLICATE BSP STACK ALLOCATION");
                    }

                    if allocation.memory_base_address != BSP_STACK_BASE
                        || allocation.memory_length != BSP_STACK_LENGTH
                    {
                        dxe_error(b"INCORRECT BSP STACK RANGE");
                    }

                    stack_found = true;
                }

                let contained_in_system_memory = ranges[..resource_count]
                    .iter()
                    .any(|&(start, end, resource_type)| {
                        resource_type == RESOURCE_SYSTEM_MEMORY
                            && allocation.memory_base_address >= start
                            && allocation_end <= end
                    });

                if !contained_in_system_memory {
                    dxe_error(b"ALLOCATION OUTSIDE SYSTEM MEMORY");
                }

                if allocation_count >= allocation_ranges.len() {
                    dxe_error(b"TOO MANY MEMORY ALLOCATIONS");
                }

                for &(previous_start, previous_end) in
                    &allocation_ranges[..allocation_count]
                {
                    if allocation.memory_base_address < previous_end
                        && previous_start < allocation_end
                    {
                        dxe_error(b"OVERLAPPING MEMORY ALLOCATIONS");
                    }
                }

                allocation_ranges[allocation_count] = (
                    allocation.memory_base_address,
                    allocation_end,
                );

                allocation_count += 1;
            }

            _ => {}
        }

        hob_count += 1;
        cursor = next;
    }

    if cursor != phit.end_of_hob_list {
        dxe_error(b"HOB WALK FAILED");
    }

    let end_header = unsafe {
        ptr::read_volatile(
            phit.end_of_hob_list as *const HobHeader
        )
    };

    if end_header.hob_type != HOB_TYPE_END
        || end_header.hob_length != 8
        || end_header.reserved != 0
    {
        dxe_error(b"INVALID END-OF-HOB-LIST");
    }

    if resource_count == 0 {
        dxe_error(b"NO RESOURCE DESCRIPTORS");
    }

    if allocation_count != 2 || !page_tables_found || !stack_found {
        dxe_error(b"MISSING OR DUPLICATE ALLOCATIONS");
    }

    let free_memory_is_valid = ranges[..resource_count]
        .iter()
        .any(|&(start, end, resource_type)| {
            resource_type == RESOURCE_SYSTEM_MEMORY
                && phit.free_memory_bottom >= start
                && phit.free_memory_top <= end
        });

    if !free_memory_is_valid {
        dxe_error(b"FREE MEMORY OUTSIDE SYSTEM MEMORY");
    }

    let mut allocator = DxeAllocator::new(
        phit.free_memory_bottom,
        phit.free_memory_top,
    ).unwrap_or_else(|| dxe_error(b"INVALID ALLOCATOR MEMORY RANGE"));

    let first_page = allocator.allocate_pages(1)
        .unwrap_or_else(|| dxe_error(b"FIRST PAGE ALLOCATION FAILED"));

    let four_pages = allocator.allocate_pages(4)
        .unwrap_or_else(|| dxe_error(b"FOUR-PAGE ALLOCATION FAILED"));

    if four_pages != first_page + PAGE_SIZE {
        dxe_error(b"ALLOCATOR SEQUENCE ERROR");
    }

    debug_write(b"DXE: PHIT VERIFIED\r\n");

    debug_write(b"DXE: HOB ENTRIES=");
    debug_hex(hob_count as u64);
    debug_write(b"\r\n");

    debug_write(b"DXE: RESOURCE DESCRIPTORS VERIFIED=");
    debug_hex(resource_count as u64);
    debug_write(b"\r\n");

    debug_write(b"DXE: MEMORY ALLOCATIONS VERIFIED=");
    debug_hex(allocation_count as u64);
    debug_write(b"\r\n");

    debug_write(b"DXE: PAGE TABLE ALLOCATION VERIFIED\r\n");
    debug_write(b"DXE: BSP STACK ALLOCATION VERIFIED\r\n");

    debug_write(b"DXE: FREE MEMORY START=0x");
    debug_hex(phit.free_memory_bottom);
    debug_write(b"\r\n");

    debug_write(b"DXE: FREE MEMORY END=0x");
    debug_hex(phit.free_memory_top);
    debug_write(b"\r\n");

    debug_write(b"DXE: HOB LIST VALIDATION PASSED\r\n");
    debug_write(b"FERRIFIRM: DXE INITIALIZATION COMPLETE\r\n");

    debug_write(b"DXE: PAGE ALLOCATOR INITIALIZED\r\n");

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
