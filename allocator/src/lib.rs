#![no_std]

pub const PAGE_SIZE: u64 = 4096;

const MAX_FREE_RANGES: usize = 128;
const MAX_ALLOCATIONS: usize = 256;

#[derive(Clone, Copy)]
struct FreeRange {
    start: u64,
    end: u64,
}

impl FreeRange {
    const EMPTY: Self = Self { start: 0, end: 0 };
}

#[derive(Clone, Copy)]
struct Allocation {
    base: u64,
    pages: u64,
    active: bool,
}

impl Allocation {
    const EMPTY: Self = Self {
        base: 0,
        pages: 0,
        active: false,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocateError {
    InvalidPageCount,
    SizeOverflow,
    OutOfMemory,
    AllocationTableFull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreeError {
    InvalidPageCount,
    SizeOverflow,
    InvalidAllocation,
    FreeRangeTableFull,
}

pub struct DxeAllocator {
    free_ranges: [FreeRange; MAX_FREE_RANGES],
    free_count: usize,
    allocations: [Allocation; MAX_ALLOCATIONS],
}

impl DxeAllocator {
    pub fn new(start: u64, end: u64) -> Option<Self> {
        if start >= end
            || start % PAGE_SIZE != 0
            || end % PAGE_SIZE != 0
        {
            return None;
        }

        let mut free_ranges = [FreeRange::EMPTY; MAX_FREE_RANGES];

        free_ranges[0] = FreeRange { start, end };

        Some(Self {
            free_ranges,
            free_count: 1,
            allocations: [Allocation::EMPTY; MAX_ALLOCATIONS],
        })
    }

    pub fn allocate_pages(
        &mut self,
        pages: u64,
    ) -> Result<u64, AllocateError> {
        if pages == 0 {
            return Err(AllocateError::InvalidPageCount);
        }

        let bytes = pages
            .checked_mul(PAGE_SIZE)
            .ok_or(AllocateError::SizeOverflow)?;

        let allocation_slot = self.allocations
            .iter()
            .position(|entry| !entry.active)
            .ok_or(AllocateError::AllocationTableFull)?;

        let range_index = self.free_ranges[..self.free_count]
            .iter()
            .position(|range| range.end - range.start >= bytes)
            .ok_or(AllocateError::OutOfMemory)?;

        let base = self.free_ranges[range_index].start;

        let end = base
            .checked_add(bytes)
            .ok_or(AllocateError::SizeOverflow)?;

        if end == self.free_ranges[range_index].end {
            self.remove_free_range(range_index);
        } else {
            self.free_ranges[range_index].start = end;
        }

        self.allocations[allocation_slot] = Allocation {
            base,
            pages,
            active: true,
        };

        Ok(base)
    }

    pub fn free_pages(
        &mut self,
        base: u64,
        pages: u64,
    ) -> Result<(), FreeError> {
        if pages == 0 || base % PAGE_SIZE != 0 {
            return Err(FreeError::InvalidPageCount);
        }

        let bytes = pages
            .checked_mul(PAGE_SIZE)
            .ok_or(FreeError::SizeOverflow)?;

        let end = base
            .checked_add(bytes)
            .ok_or(FreeError::SizeOverflow)?;

        let allocation_slot = self.allocations
            .iter()
            .position(|entry| {
                entry.active
                    && entry.base == base
                    && entry.pages == pages
            })
            .ok_or(FreeError::InvalidAllocation)?;

        self.insert_free_range(base, end)?;

        self.allocations[allocation_slot].active = false;

        Ok(())
    }

    fn insert_free_range(
        &mut self,
        start: u64,
        end: u64,
    ) -> Result<(), FreeError> {
        let mut index = 0;

        while index < self.free_count
            && self.free_ranges[index].start < start
        {
            index += 1;
        }

        if index > 0 && self.free_ranges[index - 1].end > start {
            return Err(FreeError::InvalidAllocation);
        }

        if index < self.free_count
            && end > self.free_ranges[index].start
        {
            return Err(FreeError::InvalidAllocation);
        }

        let joins_previous = index > 0
            && self.free_ranges[index - 1].end == start;

        let joins_next = index < self.free_count
            && end == self.free_ranges[index].start;

        if joins_previous && joins_next {
            self.free_ranges[index - 1].end =
                self.free_ranges[index].end;

            self.remove_free_range(index);

        } else if joins_previous {
            self.free_ranges[index - 1].end = end;

        } else if joins_next {
            self.free_ranges[index].start = start;

        } else {
            if self.free_count >= MAX_FREE_RANGES {
                return Err(FreeError::FreeRangeTableFull);
            }

            let mut i = self.free_count;

            while i > index {
                self.free_ranges[i] = self.free_ranges[i - 1];
                i -= 1;
            }

            self.free_ranges[index] = FreeRange { start, end };
            self.free_count += 1;
        }

        Ok(())
    }

    fn remove_free_range(&mut self, index: usize) {
        let mut i = index;

        while i + 1 < self.free_count {
            self.free_ranges[i] = self.free_ranges[i + 1];
            i += 1;
        }

        self.free_count -= 1;
        self.free_ranges[self.free_count] = FreeRange::EMPTY;
    }

    pub fn remaining_bytes(&self) -> u64 {
        self.free_ranges[..self.free_count]
            .iter()
            .map(|range| range.end - range.start)
            .sum()
    }

    pub fn active_allocations(&self) -> usize {
        self.allocations
            .iter()
            .filter(|entry| entry.active)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: u64 = 0x100000;

    #[test]
    fn rejects_invalid_memory_ranges() {
        assert!(DxeAllocator::new(BASE, BASE).is_none());
        assert!(DxeAllocator::new(BASE + 1, BASE + 4096).is_none());
        assert!(DxeAllocator::new(BASE + 4096, BASE).is_none());
    }

    #[test]
    fn rejects_zero_page_allocation() {
        let mut allocator =
            DxeAllocator::new(BASE, BASE + PAGE_SIZE).unwrap();

        assert!(matches!(
            allocator.allocate_pages(0),
            Err(AllocateError::InvalidPageCount)
        ));
    }

    #[test]
    fn detects_allocation_size_overflow() {
        let mut allocator =
            DxeAllocator::new(BASE, BASE + PAGE_SIZE).unwrap();

        assert!(matches!(
            allocator.allocate_pages(u64::MAX),
            Err(AllocateError::SizeOverflow)
        ));
    }

    #[test]
    fn detects_out_of_memory() {
        let mut allocator =
            DxeAllocator::new(BASE, BASE + PAGE_SIZE).unwrap();

        allocator.allocate_pages(1).unwrap();

        assert!(matches!(
            allocator.allocate_pages(1),
            Err(AllocateError::OutOfMemory)
        ));
    }

    #[test]
    fn rejects_invalid_and_double_frees() {
        let mut allocator =
            DxeAllocator::new(BASE, BASE + 4 * PAGE_SIZE).unwrap();

        let page = allocator.allocate_pages(1).unwrap();

        assert!(matches!(
            allocator.free_pages(page, 2),
            Err(FreeError::InvalidAllocation)
        ));

        allocator.free_pages(page, 1).unwrap();

        assert!(matches!(
            allocator.free_pages(page, 1),
            Err(FreeError::InvalidAllocation)
        ));
    }

    #[test]
    fn reuses_freed_memory() {
        let mut allocator =
            DxeAllocator::new(BASE, BASE + 4 * PAGE_SIZE).unwrap();

        let first = allocator.allocate_pages(2).unwrap();
        allocator.free_pages(first, 2).unwrap();

        let reused = allocator.allocate_pages(2).unwrap();

        assert_eq!(reused, first);
    }

    #[test]
    fn coalesces_adjacent_ranges_on_both_sides() {
        let mut allocator =
            DxeAllocator::new(BASE, BASE + 4 * PAGE_SIZE).unwrap();

        let a = allocator.allocate_pages(1).unwrap();
        let b = allocator.allocate_pages(1).unwrap();
        let c = allocator.allocate_pages(1).unwrap();
        let d = allocator.allocate_pages(1).unwrap();

        allocator.free_pages(a, 1).unwrap();
        allocator.free_pages(c, 1).unwrap();
        allocator.free_pages(b, 1).unwrap();
        allocator.free_pages(d, 1).unwrap();

        assert_eq!(allocator.remaining_bytes(), 4 * PAGE_SIZE);
        assert_eq!(allocator.active_allocations(), 0);
        assert_eq!(allocator.allocate_pages(4).unwrap(), BASE);
    }

    #[test]
    fn allocation_table_capacity_is_enforced() {
        let mut allocator =
            DxeAllocator::new(BASE, BASE + 257 * PAGE_SIZE).unwrap();

        for _ in 0..256 {
            allocator.allocate_pages(1).unwrap();
        }

        assert!(matches!(
            allocator.allocate_pages(1),
            Err(AllocateError::AllocationTableFull)
        ));
    }

    #[test]
    fn fragmented_ranges_remain_allocatable() {
        let mut allocator =
            DxeAllocator::new(BASE, BASE + 8 * PAGE_SIZE).unwrap();

        let mut pages = [0u64; 8];

        for page in &mut pages {
            *page = allocator.allocate_pages(1).unwrap();
        }

        for index in [1usize, 3, 5] {
            allocator.free_pages(pages[index], 1).unwrap();
        }

        assert_eq!(allocator.allocate_pages(1).unwrap(), pages[1]);
    }
}
