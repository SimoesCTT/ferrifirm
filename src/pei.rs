use core::arch::asm;
use core::ptr;

const FW_CFG_SELECTOR: u16 = 0x510;
const FW_CFG_DATA: u16 = 0x511;
const FW_CFG_SIGNATURE: u16 = 0;
const FW_CFG_FILE_DIR: u16 = 0x19;

const MAX_DIRECTORY_ENTRIES: u32 = 1024;
const MAX_E820_ENTRIES: usize = 128;
const E820_ENTRY_SIZE: usize = 20;

const HOB_BASE: usize = 0x90000;
const HOB_CAPACITY: usize = 0x4000;

const HOB_TYPE_MEMORY_ALLOCATION: u16 = 0x0002;

const PAGE_TABLE_BASE: u64 = 0x0010_0000;
const PAGE_TABLE_LENGTH: u64 = 0x0000_6000;

const BSP_STACK_BASE: u64 = 0x0011_0000;
const BSP_STACK_LENGTH: u64 = 0x0001_0000;

const FIRMWARE_ALLOC_END: u64 = 0x0012_0000;

// EFI_MEMORY_TYPE: EfiBootServicesData.
const MEMORY_TYPE_BOOT_SERVICES_DATA: u32 = 4;

const HOB_TYPE_HANDOFF: u16 = 0x0001;
const HOB_TYPE_RESOURCE_DESCRIPTOR: u16 = 0x0003;
const HOB_TYPE_END: u16 = 0xFFFF;

const HOB_VERSION: u32 = 0x000A;

const RESOURCE_SYSTEM_MEMORY: u32 = 0;
const RESOURCE_MEMORY_RESERVED: u32 = 5;

const RESOURCE_PRESENT: u32 = 0x1;
const RESOURCE_INITIALIZED: u32 = 0x2;

const RESERVED_LOW_MEMORY: u64 = 0x100000;

#[repr(C)]
#[derive(Clone, Copy)]
struct E820Entry {
    base: u64,
    length: u64,
    kind: u32,
}

impl E820Entry {
    const EMPTY: Self = Self {
        base: 0,
        length: 0,
        kind: 0,
    };
}

#[repr(C)]
#[derive(Clone, Copy)]
struct HobHeader {
    hob_type: u16,
    hob_length: u16,
    reserved: u32,
}


#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

// FerriFirm-specific page-table allocation identifier.
const FERRIFIRM_PAGE_TABLE_GUID: Guid = Guid {
    data1: 0x7d9f2b31,
    data2: 0x6a4c,
    data3: 0x4f18,
    data4: [0x9b, 0x72, 0x3e, 0x65, 0xa1, 0xc8, 0x44, 0x20],
};

// PI-defined EFI_HOB_MEMORY_ALLOC_STACK_GUID.
const BSP_STACK_GUID: Guid = Guid {
    data1: 0x4ed4bf27,
    data2: 0x4092,
    data3: 0x42e9,
    data4: [0x80, 0x7d, 0x52, 0x7b, 0x1d, 0x00, 0xc9, 0xbd],
};

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

#[repr(C)]
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
struct ResourceDescriptorHob {
    header: HobHeader,
    owner: [u8; 16],
    resource_type: u32,
    resource_attributes: u32,
    physical_start: u64,
    resource_length: u64,
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
    const HEX: &[u8] = b"0123456789ABCDEF";

    for shift in (0..16).rev() {
        let digit = ((value >> (shift * 4)) & 0xF) as usize;
        debug_write(&[HEX[digit]]);
    }
}

unsafe fn fw_cfg_select(selector: u16) {
    asm!(
        "out dx, ax",
        in("dx") FW_CFG_SELECTOR,
        in("ax") selector,
        options(nomem, nostack, preserves_flags)
    );
}

unsafe fn fw_cfg_read_byte() -> u8 {
    let value: u8;

    asm!(
        "in al, dx",
        in("dx") FW_CFG_DATA,
        out("al") value,
        options(nomem, nostack, preserves_flags)
    );

    value
}

unsafe fn fw_cfg_read_bytes(buffer: &mut [u8]) {
    for byte in buffer {
        *byte = fw_cfg_read_byte();
    }
}

fn read_be16(b: &[u8]) -> u16 {
    u16::from_be_bytes([b[0], b[1]])
}

fn read_be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

fn read_le64(b: &[u8]) -> u64 {
    u64::from_le_bytes([
        b[0], b[1], b[2], b[3],
        b[4], b[5], b[6], b[7],
    ])
}

unsafe fn check_fw_cfg() -> bool {
    fw_cfg_select(FW_CFG_SIGNATURE);

    let mut signature = [0u8; 4];
    fw_cfg_read_bytes(&mut signature);

    signature == *b"QEMU"
}

unsafe fn find_e820_file() -> Option<(u16, u32)> {
    fw_cfg_select(FW_CFG_FILE_DIR);

    let mut count_bytes = [0u8; 4];
    fw_cfg_read_bytes(&mut count_bytes);

    let count = read_be32(&count_bytes);

    if count > MAX_DIRECTORY_ENTRIES {
        return None;
    }

    for _ in 0..count {
        let mut size_bytes = [0u8; 4];
        let mut selector_bytes = [0u8; 2];
        let mut reserved = [0u8; 2];
        let mut name = [0u8; 56];

        fw_cfg_read_bytes(&mut size_bytes);
        fw_cfg_read_bytes(&mut selector_bytes);
        fw_cfg_read_bytes(&mut reserved);
        fw_cfg_read_bytes(&mut name);

        let size = read_be32(&size_bytes);
        let selector = read_be16(&selector_bytes);

        if &name[..8] == b"etc/e820" && name[8] == 0 {
            return Some((selector, size));
        }
    }

    None
}

unsafe fn read_e820_map(
    entries: &mut [E820Entry; MAX_E820_ENTRIES],
) -> Option<usize> {
    let (selector, size) = find_e820_file()?;

    if size == 0 || size as usize % E820_ENTRY_SIZE != 0 {
        return None;
    }

    let count = size as usize / E820_ENTRY_SIZE;

    if count > MAX_E820_ENTRIES {
        return None;
    }

    fw_cfg_select(selector);

    debug_write(b"\r\nPEI: E820 MEMORY MAP\r\n");

    let mut usable_total = 0u64;

    for index in 0..count {
        let mut raw = [0u8; E820_ENTRY_SIZE];
        fw_cfg_read_bytes(&mut raw);

        let base = read_le64(&raw[0..8]);
        let length = read_le64(&raw[8..16]);
        let kind = u32::from_le_bytes([
            raw[16], raw[17], raw[18], raw[19],
        ]);

        let end = base.checked_add(length)?;

        if length == 0 {
            return None;
        }

        entries[index] = E820Entry {
            base,
            length,
            kind,
        };

        if kind == 1 {
            usable_total = usable_total.checked_add(length)?;
        }

        debug_write(b"E820 BASE=0x");
        debug_hex(base);
        debug_write(b" LENGTH=0x");
        debug_hex(length);
        debug_write(b" TYPE=0x");
        debug_hex(kind as u64);
        debug_write(b"\r\n");

        let _ = end;
    }

    debug_write(b"PEI: USABLE RAM BYTES=0x");
    debug_hex(usable_total);
    debug_write(b"\r\n");

    Some(count)
}

fn write_resource_hob(
    cursor: &mut usize,
    resource_type: u32,
    attributes: u32,
    base: u64,
    length: u64,
) -> bool {
    let size = core::mem::size_of::<ResourceDescriptorHob>();

    if length == 0 || *cursor + size > HOB_CAPACITY {
        return false;
    }

    let hob = ResourceDescriptorHob {
        header: HobHeader {
            hob_type: HOB_TYPE_RESOURCE_DESCRIPTOR,
            hob_length: size as u16,
            reserved: 0,
        },
        owner: [0; 16],
        resource_type,
        resource_attributes: attributes,
        physical_start: base,
        resource_length: length,
    };

    unsafe {
        ptr::write(
            (HOB_BASE + *cursor) as *mut ResourceDescriptorHob,
            hob,
        );
    }

    *cursor += size;
    true
}


fn write_memory_allocation_hob(
    cursor: &mut usize,
    name: Guid,
    base: u64,
    length: u64,
) -> bool {
    let size = core::mem::size_of::<MemoryAllocationHob>();

    if length == 0
        || (base & 0xFFF) != 0
        || (length & 0xFFF) != 0
        || base.checked_add(length).is_none()
        || cursor.checked_add(size).map_or(true, |end| end > HOB_CAPACITY)
    {
        return false;
    }

    let hob = MemoryAllocationHob {
        header: HobHeader {
            hob_type: HOB_TYPE_MEMORY_ALLOCATION,
            hob_length: size as u16,
            reserved: 0,
        },
        allocation: MemoryAllocationHeader {
            name,
            memory_base_address: base,
            memory_length: length,
            memory_type: MEMORY_TYPE_BOOT_SERVICES_DATA,
            reserved: [0; 4],
        },
    };

    unsafe {
        ptr::write(
            (HOB_BASE + *cursor) as *mut MemoryAllocationHob,
            hob,
        );
    }

    *cursor += size;
    true
}

fn build_hob_list(
    entries: &[E820Entry; MAX_E820_ENTRIES],
    count: usize,
) -> Option<usize> {
    let mut highest_ram = 0u64;
    let mut lowest_ram = u64::MAX;

    let mut free_start = 0u64;
    let mut free_end = 0u64;
    let mut largest_free = 0u64;

    for entry in &entries[..count] {
        let end = entry.base.checked_add(entry.length)?;

        if entry.kind != 1 {
            continue;
        }

        lowest_ram = lowest_ram.min(entry.base);
        highest_ram = highest_ram.max(end);

        let start = entry.base.max(FIRMWARE_ALLOC_END);

        if end > start {
            let length = end - start;

            if length > largest_free {
                largest_free = length;
                free_start = start;
                free_end = end;
            }
        }
    }

    if highest_ram == 0 || largest_free == 0 {
        return None;
    }

    debug_write(b"\r\nPEI: BUILDING HOB LIST\r\n");

    let mut cursor = core::mem::size_of::<HandoffInfoTable>();

    // Reserve low memory, including firmware page tables,
    // stack and the HOB list itself.
    if !write_resource_hob(
        &mut cursor,
        RESOURCE_MEMORY_RESERVED,
        RESOURCE_PRESENT,
        0,
        RESERVED_LOW_MEMORY,
    ) {
        return None;
    }

    // Describe each E820 region.
    for entry in &entries[..count] {
        let end = entry.base.checked_add(entry.length)?;

        if entry.kind == 1 {
            let start = entry.base.max(RESERVED_LOW_MEMORY);

            if end <= start {
                continue;
            }

            if !write_resource_hob(
                &mut cursor,
                RESOURCE_SYSTEM_MEMORY,
                RESOURCE_PRESENT | RESOURCE_INITIALIZED,
                start,
                end - start,
            ) {
                return None;
            }
        } else {
            if !write_resource_hob(
                &mut cursor,
                RESOURCE_MEMORY_RESERVED,
                RESOURCE_PRESENT,
                entry.base,
                entry.length,
            ) {
                return None;
            }
        }
    }

    if !write_memory_allocation_hob(
        &mut cursor,
        FERRIFIRM_PAGE_TABLE_GUID,
        PAGE_TABLE_BASE,
        PAGE_TABLE_LENGTH,
    ) {
        return None;
    }

    if !write_memory_allocation_hob(
        &mut cursor,
        BSP_STACK_GUID,
        BSP_STACK_BASE,
        BSP_STACK_LENGTH,
    ) {
        return None;
    }

    let end_hob_address = HOB_BASE + cursor;

    let end_header = HobHeader {
        hob_type: HOB_TYPE_END,
        hob_length: core::mem::size_of::<HobHeader>() as u16,
        reserved: 0,
    };

    if cursor + core::mem::size_of::<HobHeader>() > HOB_CAPACITY {
        return None;
    }

    unsafe {
        ptr::write(
            end_hob_address as *mut HobHeader,
            end_header,
        );
    }

    cursor += core::mem::size_of::<HobHeader>();

    let phit = HandoffInfoTable {
        header: HobHeader {
            hob_type: HOB_TYPE_HANDOFF,
            hob_length: core::mem::size_of::<HandoffInfoTable>() as u16,
            reserved: 0,
        },
        version: HOB_VERSION,
        boot_mode: 0,
        memory_top: highest_ram,
        memory_bottom: lowest_ram,
        free_memory_top: free_end,
        free_memory_bottom: free_start,
        end_of_hob_list: end_hob_address as u64,
    };

    unsafe {
        ptr::write(HOB_BASE as *mut HandoffInfoTable, phit);
    }

    debug_write(b"PEI: HOB BASE=0x");
    debug_hex(HOB_BASE as u64);

    debug_write(b"\r\nPEI: HOB END=0x");
    debug_hex(end_hob_address as u64);

    debug_write(b"\r\nPEI: HOB BYTES=0x");
    debug_hex(cursor as u64);

    debug_write(b"\r\nPEI: FREE MEMORY START=0x");
    debug_hex(free_start);

    debug_write(b"\r\nPEI: FREE MEMORY END=0x");
    debug_hex(free_end);

    debug_write(b"\r\n");

    Some(cursor)
}


fn hob_validation_error(reason: &[u8]) -> bool {
    debug_write(b"PEI HOB VALIDATION FAILED: ");
    debug_write(reason);
    debug_write(b"\r\n");
    false
}

fn validate_hob_list() -> bool {
    use core::mem::size_of;

    debug_write(b"\r\nPEI: VALIDATING HOB LIST\r\n");

    if HOB_BASE % 8 != 0 {
        return hob_validation_error(b"HOB BASE IS NOT 8-BYTE ALIGNED");
    }

    if size_of::<HobHeader>() != 8
        || size_of::<HandoffInfoTable>() != 56
        || size_of::<ResourceDescriptorHob>() != 48
        || size_of::<MemoryAllocationHob>() != 48
        || size_of::<Guid>() != 16
    {
        return hob_validation_error(b"STRUCTURE SIZE MISMATCH");
    }

    let phit = unsafe {
        ptr::read_unaligned(HOB_BASE as *const HandoffInfoTable)
    };

    if phit.header.hob_type != HOB_TYPE_HANDOFF {
        return hob_validation_error(b"PHIT IS NOT FIRST");
    }

    if phit.header.hob_length as usize != size_of::<HandoffInfoTable>() {
        return hob_validation_error(b"INVALID PHIT LENGTH");
    }

    if phit.header.reserved != 0 {
        return hob_validation_error(b"PHIT RESERVED FIELD IS NOT ZERO");
    }

    if phit.version != HOB_VERSION {
        return hob_validation_error(b"INCORRECT PI HOB VERSION");
    }

    if phit.memory_bottom >= phit.memory_top {
        return hob_validation_error(b"INVALID PHIT MEMORY RANGE");
    }

    if phit.free_memory_bottom != FIRMWARE_ALLOC_END
        || phit.free_memory_bottom >= phit.free_memory_top
        || phit.free_memory_bottom < phit.memory_bottom
        || phit.free_memory_top > phit.memory_top
    {
        return hob_validation_error(b"INVALID PHIT FREE MEMORY RANGE");
    }

    if (phit.memory_bottom & 0xFFF) != 0
        || (phit.memory_top & 0xFFF) != 0
        || (phit.free_memory_bottom & 0xFFF) != 0
        || (phit.free_memory_top & 0xFFF) != 0
    {
        return hob_validation_error(b"PHIT MEMORY BOUNDARY NOT PAGE ALIGNED");
    }

    debug_write(b"PEI: PHIT VERIFIED\r\n");

    let mut offset = size_of::<HandoffInfoTable>();
    let mut resource_count = 0usize;
    let mut allocation_count = 0usize;

    let mut ranges = [(0u64, 0u64, 0u32); MAX_E820_ENTRIES + 1];
    let mut allocation_ranges = [(0u64, 0u64); 2];

    let mut page_tables_found = false;
    let mut stack_found = false;

    loop {
        if offset % 8 != 0 {
            return hob_validation_error(b"HOB IS NOT 8-BYTE ALIGNED");
        }

        let header_end = match offset.checked_add(size_of::<HobHeader>()) {
            Some(value) if value <= HOB_CAPACITY => value,
            _ => return hob_validation_error(b"HOB HEADER EXCEEDS CAPACITY"),
        };

        let address = match HOB_BASE.checked_add(offset) {
            Some(value) => value,
            None => return hob_validation_error(b"HOB ADDRESS OVERFLOW"),
        };

        let header = unsafe {
            ptr::read_unaligned(address as *const HobHeader)
        };

        let length = header.hob_length as usize;

        if header.reserved != 0 {
            return hob_validation_error(b"HOB RESERVED FIELD IS NOT ZERO");
        }

        if length < size_of::<HobHeader>() || length % 8 != 0 {
            return hob_validation_error(b"INVALID HOB LENGTH OR ALIGNMENT");
        }

        let next_offset = match offset.checked_add(length) {
            Some(value) if value <= HOB_CAPACITY => value,
            _ => return hob_validation_error(b"HOB EXCEEDS LIST CAPACITY"),
        };

        match header.hob_type {
            HOB_TYPE_END => {
                if length != size_of::<HobHeader>() {
                    return hob_validation_error(b"INVALID END HOB LENGTH");
                }

                if phit.end_of_hob_list != address as u64 {
                    return hob_validation_error(b"PHIT END POINTER MISMATCH");
                }

                if resource_count == 0 {
                    return hob_validation_error(b"NO RESOURCE DESCRIPTORS FOUND");
                }

                if allocation_count != 2 || !page_tables_found || !stack_found {
                    return hob_validation_error(b"MISSING OR DUPLICATE MEMORY ALLOCATION HOB");
                }

                debug_write(b"PEI: RESOURCE DESCRIPTORS VERIFIED=");
                debug_hex(resource_count as u64);
                debug_write(b"\r\n");

                debug_write(b"PEI: MEMORY ALLOCATIONS VERIFIED=");
                debug_hex(allocation_count as u64);
                debug_write(b"\r\n");

                debug_write(b"PEI: PAGE TABLE ALLOCATION VERIFIED\r\n");
                debug_write(b"PEI: BSP STACK ALLOCATION VERIFIED\r\n");
                debug_write(b"PEI: END-OF-HOB-LIST VERIFIED\r\n");
                debug_write(b"PEI: HOB LIST VALIDATION PASSED\r\n");

                return true;
            }

            HOB_TYPE_HANDOFF => {
                return hob_validation_error(b"UNEXPECTED SECOND PHIT");
            }

            HOB_TYPE_RESOURCE_DESCRIPTOR => {
                if length != size_of::<ResourceDescriptorHob>() {
                    return hob_validation_error(b"INVALID RESOURCE HOB LENGTH");
                }

                let resource = unsafe {
                    ptr::read_unaligned(address as *const ResourceDescriptorHob)
                };

                if resource.resource_type != RESOURCE_SYSTEM_MEMORY
                    && resource.resource_type != RESOURCE_MEMORY_RESERVED
                {
                    return hob_validation_error(b"UNSUPPORTED RESOURCE TYPE");
                }

                if resource.resource_attributes & RESOURCE_PRESENT == 0 {
                    return hob_validation_error(b"RESOURCE NOT MARKED PRESENT");
                }

                if resource.resource_type == RESOURCE_SYSTEM_MEMORY
                    && resource.resource_attributes & RESOURCE_INITIALIZED == 0
                {
                    return hob_validation_error(b"SYSTEM MEMORY NOT INITIALIZED");
                }

                if resource.resource_length == 0 {
                    return hob_validation_error(b"EMPTY RESOURCE RANGE");
                }

                let range_end = match resource.physical_start
                    .checked_add(resource.resource_length)
                {
                    Some(value) => value,
                    None => return hob_validation_error(b"RESOURCE RANGE OVERFLOW"),
                };

                if resource_count >= ranges.len() {
                    return hob_validation_error(b"TOO MANY RESOURCE DESCRIPTORS");
                }

                for &(previous_start, previous_end, _) in &ranges[..resource_count] {
                    if resource.physical_start < previous_end
                        && previous_start < range_end
                    {
                        return hob_validation_error(b"OVERLAPPING RESOURCE RANGES");
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
                if length != size_of::<MemoryAllocationHob>() {
                    return hob_validation_error(b"INVALID MEMORY ALLOCATION HOB LENGTH");
                }

                let allocation_hob = unsafe {
                    ptr::read_unaligned(address as *const MemoryAllocationHob)
                };

                let allocation = allocation_hob.allocation;

                if allocation.reserved != [0; 4] {
                    return hob_validation_error(b"ALLOCATION RESERVED FIELD IS NOT ZERO");
                }

                if allocation.memory_length == 0
                    || (allocation.memory_base_address & 0xFFF) != 0
                    || (allocation.memory_length & 0xFFF) != 0
                {
                    return hob_validation_error(b"INVALID ALLOCATION RANGE OR ALIGNMENT");
                }

                if allocation.memory_type != MEMORY_TYPE_BOOT_SERVICES_DATA {
                    return hob_validation_error(b"INCORRECT ALLOCATION MEMORY TYPE");
                }

                let allocation_end = match allocation.memory_base_address
                    .checked_add(allocation.memory_length)
                {
                    Some(value) => value,
                    None => return hob_validation_error(b"ALLOCATION RANGE OVERFLOW"),
                };

                let is_page_tables =
                    allocation.name == FERRIFIRM_PAGE_TABLE_GUID;
                let is_stack =
                    allocation.name == BSP_STACK_GUID;

                if !is_page_tables && !is_stack {
                    return hob_validation_error(b"UNKNOWN MEMORY ALLOCATION GUID");
                }

                if is_page_tables {
                    if page_tables_found {
                        return hob_validation_error(b"DUPLICATE PAGE TABLE ALLOCATION");
                    }

                    if allocation.memory_base_address != PAGE_TABLE_BASE
                        || allocation.memory_length != PAGE_TABLE_LENGTH
                    {
                        return hob_validation_error(b"INCORRECT PAGE TABLE ALLOCATION RANGE");
                    }

                    page_tables_found = true;
                }

                if is_stack {
                    if stack_found {
                        return hob_validation_error(b"DUPLICATE BSP STACK ALLOCATION");
                    }

                    if allocation.memory_base_address != BSP_STACK_BASE
                        || allocation.memory_length != BSP_STACK_LENGTH
                    {
                        return hob_validation_error(b"INCORRECT BSP STACK ALLOCATION RANGE");
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
                    return hob_validation_error(b"ALLOCATION OUTSIDE SYSTEM MEMORY");
                }

                if allocation_count >= allocation_ranges.len() {
                    return hob_validation_error(b"TOO MANY MEMORY ALLOCATIONS");
                }

                for &(previous_start, previous_end) in
                    &allocation_ranges[..allocation_count]
                {
                    if allocation.memory_base_address < previous_end
                        && previous_start < allocation_end
                    {
                        return hob_validation_error(b"OVERLAPPING MEMORY ALLOCATIONS");
                    }
                }

                allocation_ranges[allocation_count] = (
                    allocation.memory_base_address,
                    allocation_end,
                );
                allocation_count += 1;
            }

            // Unknown HOB types are skipped using their validated length.
            _ => {}
        }

        offset = next_offset;

        if offset == header_end {
            return hob_validation_error(b"INVALID HOB WALK");
        }
    }
}

pub fn pei_main() -> ! {
    debug_write(b"\r\nFERRIFIRM: ENTERING PEI\r\n");

    let mut entries = [E820Entry::EMPTY; MAX_E820_ENTRIES];

    unsafe {
        if !check_fw_cfg() {
            debug_write(b"PEI ERROR: FW_CFG NOT DETECTED\r\n");
            halt();
        }

        debug_write(b"PEI: QEMU FW_CFG DETECTED\r\n");

        let count = match read_e820_map(&mut entries) {
            Some(count) => count,
            None => {
                debug_write(b"PEI ERROR: E820 DISCOVERY FAILED\r\n");
                halt();
            }
        };

        if build_hob_list(&entries, count).is_none() {
            debug_write(b"PEI ERROR: HOB CONSTRUCTION FAILED\r\n");
            halt();
        }

        if !validate_hob_list() {
            halt();
        }
    }

    debug_write(b"\r\nPEI: MEMORY DISCOVERY COMPLETE\r\n");
    debug_write(b"PEI: HOB LIST CONSTRUCTION COMPLETE\r\n");
    debug_write(b"PEI: HANDING OFF TO DXE\r\n");

    crate::dxe::dxe_main(HOB_BASE as u64);
}

fn halt() -> ! {
    loop {
        unsafe {
            asm!("cli", "hlt", options(nomem, nostack));
        }
    }
}
