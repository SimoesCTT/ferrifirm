pub(super) const PAGE_SIZE: u64 = 4096;

pub(super) struct DxeAllocator {
    next: u64,
    end: u64,
}

impl DxeAllocator {
    pub(super) fn new(start: u64, end: u64) -> Option<Self> {
        if start >= end
            || start % PAGE_SIZE != 0
            || end % PAGE_SIZE != 0
        {
            return None;
        }

        Some(Self { next: start, end })
    }

    pub(super) fn allocate_pages(&mut self, pages: u64) -> Option<u64> {
        if pages == 0 {
            return None;
        }

        let bytes = pages.checked_mul(PAGE_SIZE)?;
        let start = self.next.checked_add(PAGE_SIZE - 1)?
            & !(PAGE_SIZE - 1);
        let end = start.checked_add(bytes)?;

        if end > self.end {
            return None;
        }

        self.next = end;
        Some(start)
    }

    pub(super) fn remaining_bytes(&self) -> u64 {
        self.end - self.next
    }
}
