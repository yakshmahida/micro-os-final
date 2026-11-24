//! 64 KiB Bump Allocator for Titanium Micro-OS.
//!
//! Manages a contiguous 65,536-byte heap arena with power-of-two alignment,
//! strict boundary checking, OOM safety, and full arena reset.

use std::alloc::{alloc_zeroed, dealloc, Layout};

/// Total size of the managed heap arena (64 KiB).
pub const HEAP_SIZE: usize = 64 * 1024;
const HEAP_ALIGN: usize = 4096;

/// Snapshot of current allocator utilization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocatorStats {
    pub used: usize,
    pub free: usize,
    pub total: usize,
    pub allocations: usize,
}

/// A monotonic bump allocator managing a fixed 64 KiB heap memory region.
pub struct BumpAllocator {
    raw_ptr: *mut u8,
    layout: Layout,
    heap_start: usize,
    heap_end: usize,
    next: usize,
    allocations: usize,
}

impl BumpAllocator {
    /// Creates a new `BumpAllocator` allocating a clean 64 KiB heap aligned to 4096 bytes.
    pub fn new() -> Self {
        let layout = Layout::from_size_align(HEAP_SIZE, HEAP_ALIGN)
            .expect("Valid heap layout configuration");
        let raw_ptr = unsafe { alloc_zeroed(layout) };
        if raw_ptr.is_null() {
            panic!("Failed to allocate 64 KiB heap memory for BumpAllocator");
        }
        let heap_start = raw_ptr as usize;
        let heap_end = heap_start + HEAP_SIZE;

        Self {
            raw_ptr,
            layout,
            heap_start,
            heap_end,
            next: heap_start,
            allocations: 0,
        }
    }

    /// Allocates `size` bytes aligned to `align` boundary.
    ///
    /// Alignment must be a non-zero power of two.
    /// Returns `Some(*mut u8)` on success, or `None` if `size == 0`,
    /// alignment is invalid, or the allocation exceeds heap capacity.
    pub fn allocate(&mut self, size: usize, align: usize) -> Option<*mut u8> {
        if size == 0 || !align.is_power_of_two() {
            return None;
        }

        // Align formula: (next + align - 1) & !(align - 1)
        let alloc_start = (self.next.checked_add(align - 1)?) & !(align - 1);
        let alloc_end = alloc_start.checked_add(size)?;

        if alloc_end > self.heap_end {
            return None;
        }

        self.next = alloc_end;
        self.allocations += 1;
        Some(alloc_start as *mut u8)
    }

    /// Resets the allocator pointer to the base of the heap, reclaiming all memory.
    pub fn reset(&mut self) {
        self.next = self.heap_start;
        self.allocations = 0;
        unsafe {
            core::ptr::write_bytes(self.raw_ptr, 0, HEAP_SIZE);
        }
    }

    /// Returns the number of bytes currently allocated (including alignment padding).
    pub fn used(&self) -> usize {
        self.next - self.heap_start
    }

    /// Returns the number of bytes remaining free in the heap.
    pub fn free(&self) -> usize {
        self.heap_end - self.next
    }

    /// Returns the total heap size in bytes (65,536).
    pub fn total(&self) -> usize {
        HEAP_SIZE
    }

    /// Returns the total number of successful allocations made since creation or last reset.
    pub fn allocations(&self) -> usize {
        self.allocations
    }

    /// Returns a structured summary of allocator statistics.
    pub fn stats(&self) -> AllocatorStats {
        AllocatorStats {
            used: self.used(),
            free: self.free(),
            total: self.total(),
            allocations: self.allocations,
        }
    }

    /// Returns current bump pointer address.
    pub fn next_ptr(&self) -> *mut u8 {
        self.next as *mut u8
    }
}

impl Default for BumpAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for BumpAllocator {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.raw_ptr, self.layout);
        }
    }
}

unsafe impl Send for BumpAllocator {}
unsafe impl Sync for BumpAllocator {}
