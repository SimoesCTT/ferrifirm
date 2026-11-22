pub(super) const HOB_TYPE_HANDOFF: u16 = 0x0001;
pub(super) const HOB_TYPE_MEMORY_ALLOCATION: u16 = 0x0002;
pub(super) const HOB_TYPE_RESOURCE_DESCRIPTOR: u16 = 0x0003;
pub(super) const HOB_TYPE_END: u16 = 0xFFFF;

pub(super) const HOB_VERSION: u32 = 0x000A;
pub(super) const HOB_CAPACITY: u64 = 0x4000;

pub(super) const RESOURCE_SYSTEM_MEMORY: u32 = 0;
pub(super) const RESOURCE_MEMORY_RESERVED: u32 = 5;

pub(super) const RESOURCE_PRESENT: u32 = 0x1;
pub(super) const RESOURCE_INITIALIZED: u32 = 0x2;

pub(super) const MEMORY_TYPE_BOOT_SERVICES_DATA: u32 = 4;

pub(super) const PAGE_TABLE_BASE: u64 = 0x0010_0000;
pub(super) const PAGE_TABLE_LENGTH: u64 = 0x0000_6000;

pub(super) const BSP_STACK_BASE: u64 = 0x0011_0000;
pub(super) const BSP_STACK_LENGTH: u64 = 0x0001_0000;

pub(super) const MAX_RESOURCE_DESCRIPTORS: usize = 129;

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct HobHeader {
    pub(super) hob_type: u16,
    pub(super) hob_length: u16,
    pub(super) reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct Guid {
    pub(super) data1: u32,
    pub(super) data2: u16,
    pub(super) data3: u16,
    pub(super) data4: [u8; 8],
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

pub(super) const FERRIFIRM_PAGE_TABLE_GUID: Guid = Guid {
    data1: 0x7d9f2b31,
    data2: 0x6a4c,
    data3: 0x4f18,
    data4: [0x9b, 0x72, 0x3e, 0x65, 0xa1, 0xc8, 0x44, 0x20],
};

pub(super) const BSP_STACK_GUID: Guid = Guid {
    data1: 0x4ed4bf27,
    data2: 0x4092,
    data3: 0x42e9,
    data4: [0x80, 0x7d, 0x52, 0x7b, 0x1d, 0x00, 0xc9, 0xbd],
};

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct HandoffInfoTable {
    pub(super) header: HobHeader,
    pub(super) version: u32,
    pub(super) boot_mode: u32,
    pub(super) memory_top: u64,
    pub(super) memory_bottom: u64,
    pub(super) free_memory_top: u64,
    pub(super) free_memory_bottom: u64,
    pub(super) end_of_hob_list: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct ResourceDescriptorHob {
    pub(super) header: HobHeader,
    pub(super) owner: [u8; 16],
    pub(super) resource_type: u32,
    pub(super) resource_attributes: u32,
    pub(super) physical_start: u64,
    pub(super) resource_length: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct MemoryAllocationHeader {
    pub(super) name: Guid,
    pub(super) memory_base_address: u64,
    pub(super) memory_length: u64,
    pub(super) memory_type: u32,
    pub(super) reserved: [u8; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct MemoryAllocationHob {
    pub(super) header: HobHeader,
    pub(super) allocation: MemoryAllocationHeader,
}


use core::ptr;

pub(super) struct ValidatedMemoryMap {
    pub(super) free_start: u64,
    pub(super) free_end: u64,
    pub(super) resource_count: usize,
    pub(super) allocation_count: usize,
    pub(super) hob_count: usize,
}

pub(super) fn validate(
    hob_address: u64,
) -> Result<ValidatedMemoryMap, &'static [u8]> {
    if hob_address == 0 || hob_address & 7 != 0 {
        return Err(b"INVALID HOB ADDRESS");
    }

    if core::mem::size_of::<HobHeader>() != 8
        || core::mem::size_of::<HandoffInfoTable>() != 56
        || core::mem::size_of::<ResourceDescriptorHob>() != 48
        || core::mem::size_of::<MemoryAllocationHob>() != 48
        || core::mem::size_of::<Guid>() != 16
    {
        return Err(b"STRUCTURE SIZE MISMATCH");
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
        return Err(b"INVALID PHIT");
    }

    if phit.memory_bottom >= phit.memory_top
        || phit.free_memory_bottom >= phit.free_memory_top
        || phit.free_memory_bottom < phit.memory_bottom
        || phit.free_memory_top > phit.memory_top
    {
        return Err(b"INVALID PHIT MEMORY RANGE");
    }

    if (phit.memory_bottom & 0xFFF) != 0
        || (phit.memory_top & 0xFFF) != 0
        || (phit.free_memory_bottom & 0xFFF) != 0
        || (phit.free_memory_top & 0xFFF) != 0
    {
        return Err(b"UNALIGNED PHIT MEMORY RANGE");
    }

    let minimum_end = match hob_address.checked_add(56) {
        Some(value) => value,
        None => return Err(b"HOB ADDRESS OVERFLOW"),
    };

    let maximum_end = match hob_address.checked_add(HOB_CAPACITY - 8) {
        Some(value) => value,
        None => return Err(b"HOB CAPACITY OVERFLOW"),
    };

    if phit.end_of_hob_list < minimum_end
        || phit.end_of_hob_list > maximum_end
        || phit.end_of_hob_list & 7 != 0
    {
        return Err(b"INVALID HOB END ADDRESS");
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
            return Err(b"INVALID HOB HEADER");
        }

        let next = match cursor.checked_add(length) {
            Some(value) => value,
            None => return Err(b"HOB LENGTH OVERFLOW"),
        };

        if next > phit.end_of_hob_list {
            return Err(b"HOB EXCEEDS END ADDRESS");
        }

        match header.hob_type {
            HOB_TYPE_HANDOFF | HOB_TYPE_END => {
                return Err(b"UNEXPECTED HOB TYPE");
            }

            HOB_TYPE_RESOURCE_DESCRIPTOR => {
                if length != 48 {
                    return Err(b"INVALID RESOURCE HOB LENGTH");
                }

                let resource = unsafe {
                    ptr::read_volatile(
                        cursor as *const ResourceDescriptorHob
                    )
                };

                if resource.resource_type != RESOURCE_SYSTEM_MEMORY
                    && resource.resource_type != RESOURCE_MEMORY_RESERVED
                {
                    return Err(b"UNSUPPORTED RESOURCE TYPE");
                }

                if resource.resource_attributes & RESOURCE_PRESENT == 0 {
                    return Err(b"RESOURCE NOT PRESENT");
                }

                if resource.resource_type == RESOURCE_SYSTEM_MEMORY
                    && resource.resource_attributes & RESOURCE_INITIALIZED == 0
                {
                    return Err(b"SYSTEM MEMORY NOT INITIALIZED");
                }

                if resource.resource_length == 0
                    || (resource.physical_start & 0xFFF) != 0
                    || (resource.resource_length & 0xFFF) != 0
                {
                    return Err(b"INVALID RESOURCE RANGE");
                }

                let range_end = match resource.physical_start
                    .checked_add(resource.resource_length)
                {
                    Some(value) => value,
                    None => return Err(b"RESOURCE RANGE OVERFLOW"),
                };

                if resource_count >= ranges.len() {
                    return Err(b"TOO MANY RESOURCE DESCRIPTORS");
                }

                for &(previous_start, previous_end, _) in
                    &ranges[..resource_count]
                {
                    if resource.physical_start < previous_end
                        && previous_start < range_end
                    {
                        return Err(b"OVERLAPPING RESOURCE RANGES");
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
                    return Err(b"INVALID ALLOCATION HOB LENGTH");
                }

                let allocation_hob = unsafe {
                    ptr::read_volatile(
                        cursor as *const MemoryAllocationHob
                    )
                };

                let allocation = allocation_hob.allocation;

                if allocation.reserved != [0; 4] {
                    return Err(b"ALLOCATION RESERVED FIELD NOT ZERO");
                }

                if allocation.memory_type != MEMORY_TYPE_BOOT_SERVICES_DATA {
                    return Err(b"INCORRECT ALLOCATION MEMORY TYPE");
                }

                if allocation.memory_length == 0
                    || (allocation.memory_base_address & 0xFFF) != 0
                    || (allocation.memory_length & 0xFFF) != 0
                {
                    return Err(b"INVALID ALLOCATION RANGE");
                }

                let allocation_end = match allocation.memory_base_address
                    .checked_add(allocation.memory_length)
                {
                    Some(value) => value,
                    None => return Err(b"ALLOCATION RANGE OVERFLOW"),
                };

                let is_page_tables =
                    allocation.name == FERRIFIRM_PAGE_TABLE_GUID;
                let is_stack = allocation.name == BSP_STACK_GUID;

                if !is_page_tables && !is_stack {
                    return Err(b"UNKNOWN ALLOCATION GUID");
                }

                if is_page_tables {
                    if page_tables_found {
                        return Err(b"DUPLICATE PAGE TABLE ALLOCATION");
                    }

                    if allocation.memory_base_address != PAGE_TABLE_BASE
                        || allocation.memory_length != PAGE_TABLE_LENGTH
                    {
                        return Err(b"INCORRECT PAGE TABLE RANGE");
                    }

                    page_tables_found = true;
                }

                if is_stack {
                    if stack_found {
                        return Err(b"DUPLICATE BSP STACK ALLOCATION");
                    }

                    if allocation.memory_base_address != BSP_STACK_BASE
                        || allocation.memory_length != BSP_STACK_LENGTH
                    {
                        return Err(b"INCORRECT BSP STACK RANGE");
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
                    return Err(b"ALLOCATION OUTSIDE SYSTEM MEMORY");
                }

                if allocation_count >= allocation_ranges.len() {
                    return Err(b"TOO MANY MEMORY ALLOCATIONS");
                }

                for &(previous_start, previous_end) in
                    &allocation_ranges[..allocation_count]
                {
                    if allocation.memory_base_address < previous_end
                        && previous_start < allocation_end
                    {
                        return Err(b"OVERLAPPING MEMORY ALLOCATIONS");
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
        return Err(b"HOB WALK FAILED");
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
        return Err(b"INVALID END-OF-HOB-LIST");
    }

    if resource_count == 0 {
        return Err(b"NO RESOURCE DESCRIPTORS");
    }

    if allocation_count != 2 || !page_tables_found || !stack_found {
        return Err(b"MISSING OR DUPLICATE ALLOCATIONS");
    }

    let free_memory_is_valid = ranges[..resource_count]
        .iter()
        .any(|&(start, end, resource_type)| {
            resource_type == RESOURCE_SYSTEM_MEMORY
                && phit.free_memory_bottom >= start
                && phit.free_memory_top <= end
        });

    if !free_memory_is_valid {
        return Err(b"FREE MEMORY OUTSIDE SYSTEM MEMORY");
    }


    Ok(ValidatedMemoryMap {
        free_start: phit.free_memory_bottom,
        free_end: phit.free_memory_top,
        resource_count,
        allocation_count,
        hob_count,
    })
}
