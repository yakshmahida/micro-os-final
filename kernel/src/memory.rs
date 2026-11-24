//! 64 KiB Bump Allocator for Titanium Micro-OS Bare-Metal Kernel.
//!
//! Manages a static 65,536-byte heap arena with power-of-two alignment,
//! strict boundary checking, OOM safety, and full arena reset.

use crate::sync::SpinLock;

/// Total size of the managed heap arena (64 KiB).
pub const HEAP_SIZE: usize = 64 * 1024;
pub const HEAP_ALIGN: usize = 4096;

#[repr(align(4096))]
pub struct HeapStorage(pub [u8; HEAP_SIZE]);

pub static mut HEAP: HeapStorage = HeapStorage([0; HEAP_SIZE]);

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
    heap_start: usize,
    heap_end: usize,
    next: usize,
    allocations: usize,
}

impl BumpAllocator {
    /// Creates an uninitialized `BumpAllocator` suitable for `const` contexts.
    pub const fn uninit() -> Self {
        Self {
            heap_start: 0,
            heap_end: 0,
            next: 0,
            allocations: 0,
        }
    }

    /// Creates and initializes a `BumpAllocator` targeting the static heap buffer.
    pub fn new() -> Self {
        let heap_start = unsafe { core::ptr::addr_of_mut!(HEAP.0) as *mut u8 as usize };
        let heap_end = heap_start + HEAP_SIZE;
        Self {
            heap_start,
            heap_end,
            next: heap_start,
            allocations: 0,
        }
    }

    /// Initializes or re-binds allocator state to the static heap buffer.
    pub fn init(&mut self) {
        let heap_start = unsafe { core::ptr::addr_of_mut!(HEAP.0) as *mut u8 as usize };
        self.heap_start = heap_start;
        self.heap_end = heap_start + HEAP_SIZE;
        self.next = heap_start;
        self.allocations = 0;
    }

    /// Allocates `size` bytes aligned to `align` boundary.
    ///
    /// Alignment must be a non-zero power of two.
    /// Returns `Some(*mut u8)` on success, or `None` if `size == 0`,
    /// alignment is invalid, or the allocation exceeds heap capacity.
    pub fn allocate(&mut self, size: usize, align: usize) -> Option<*mut u8> {
        if self.heap_start == 0 {
            self.init();
        }
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
        if self.heap_start == 0 {
            self.init();
        }
        self.next = self.heap_start;
        self.allocations = 0;
        unsafe {
            core::ptr::write_bytes(self.heap_start as *mut u8, 0, HEAP_SIZE);
        }
    }

    /// Returns the number of bytes currently allocated (including alignment padding).
    pub fn used(&self) -> usize {
        if self.heap_start == 0 {
            0
        } else {
            self.next - self.heap_start
        }
    }

    /// Returns the number of bytes remaining free in the heap.
    pub fn free(&self) -> usize {
        if self.heap_start == 0 {
            HEAP_SIZE
        } else {
            self.heap_end - self.next
        }
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
        if self.heap_start == 0 {
            unsafe { core::ptr::addr_of_mut!(HEAP.0) as *mut u8 }
        } else {
            self.next as *mut u8
        }
    }
}

impl Default for BumpAllocator {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl Send for BumpAllocator {}
unsafe impl Sync for BumpAllocator {}

/// Global system bump allocator wrapped in a SpinLock.
pub static ALLOCATOR: SpinLock<BumpAllocator> = SpinLock::new(BumpAllocator::uninit());

/// Initializes the global kernel bump allocator.
pub fn init() {
    ALLOCATOR.lock().init();
}

/// Allocates memory from the global bump allocator.
pub fn allocate(size: usize, align: usize) -> Option<*mut u8> {
    ALLOCATOR.lock().allocate(size, align)
}

/// Resets the global bump allocator.
pub fn reset() {
    ALLOCATOR.lock().reset();
}

/// Returns a snapshot of global allocator statistics.
pub fn stats() -> AllocatorStats {
    ALLOCATOR.lock().stats()
}

pub fn used() -> usize {
    ALLOCATOR.lock().used()
}

pub fn free() -> usize {
    ALLOCATOR.lock().free()
}

pub fn total() -> usize {
    HEAP_SIZE
}
