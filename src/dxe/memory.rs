pub(super) const PAGE_SIZE: u64 = 4096;

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

pub(super) enum AllocateError {
    InvalidPageCount,
    SizeOverflow,
    OutOfMemory,
    AllocationTableFull,
}

pub(super) enum FreeError {
    InvalidPageCount,
    SizeOverflow,
    InvalidAllocation,
    FreeRangeTableFull,
}

pub(super) struct DxeAllocator {
    free_ranges: [FreeRange; MAX_FREE_RANGES],
    free_count: usize,
    allocations: [Allocation; MAX_ALLOCATIONS],
}

impl DxeAllocator {
    pub(super) fn new(start: u64, end: u64) -> Option<Self> {
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

    pub(super) fn allocate_pages(
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

    pub(super) fn free_pages(
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

    pub(super) fn remaining_bytes(&self) -> u64 {
        self.free_ranges[..self.free_count]
            .iter()
            .map(|range| range.end - range.start)
            .sum()
    }

    pub(super) fn active_allocations(&self) -> usize {
        self.allocations
            .iter()
            .filter(|entry| entry.active)
            .count()
    }
}
