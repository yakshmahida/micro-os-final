//! Tier 1 & Tier 2 E2E Tests for R3: 64 KiB Bump Allocator & SpinLock Synchronization.
//! CS2001 OS Capstone | Student: Yakshendrasinh R Mahida (USN: 25BTRCL288)

use std::sync::Arc;
use std::thread;
use micro_os::memory::BumpAllocator;
use micro_os::sync::SpinLock;

// ============================================================================
// Tier 1: Feature Coverage (Happy Path)
// ============================================================================

/// `TC-T1-17`: 64 KiB Heap Allocation & Tracking.
/// Validates allocation updates `used` and `free` counters accurately (used + free = 65,536).
#[test]
fn t1_mem_17_64kib_heap_allocation_and_tracking() {
    let mut alloc = BumpAllocator::new();
    assert_eq!(alloc.total(), 65536, "Total heap must be 64 KiB (65536 bytes)");
    assert_eq!(alloc.used(), 0);
    assert_eq!(alloc.free(), 65536);

    // Allocate 1024 bytes (8-byte aligned)
    let ptr = alloc.allocate(1024, 8);
    assert!(ptr.is_some(), "Allocation of 1024 bytes must succeed");
    assert_eq!(alloc.used(), 1024);
    assert_eq!(alloc.free(), 65536 - 1024);
    assert_eq!(alloc.used() + alloc.free(), alloc.total());

    // Allocate another 2048 bytes (16-byte aligned)
    let ptr2 = alloc.allocate(2048, 16);
    assert!(ptr2.is_some(), "Allocation of 2048 bytes must succeed");
    assert_eq!(alloc.used(), 1024 + 2048);
    assert_eq!(alloc.free(), 65536 - 3072);
    assert_eq!(alloc.used() + alloc.free(), alloc.total());
}

/// `TC-T1-18`: Power-of-2 Alignment Enforcement.
/// Validates addresses satisfy 1, 2, 4, 8, 16, 64-byte alignment.
#[test]
fn t1_mem_18_power_of_2_alignment_enforcement() {
    let mut alloc = BumpAllocator::new();
    let alignments = [1usize, 2, 4, 8, 16, 32, 64, 128];

    // Allocate an odd number of bytes first to throw off natural alignment
    alloc.allocate(3, 1);

    for &align in &alignments {
        let ptr = alloc.allocate(16, align).expect("Allocation should succeed");
        let addr = ptr as usize;
        assert_eq!(
            addr % align,
            0,
            "Address 0x{:X} must be aligned to {} bytes",
            addr,
            align
        );
    }
}

/// `TC-T1-19`: Contiguous Monotonic Allocations.
/// Validates sequential allocations increase monotonically without overlap.
#[test]
fn t1_mem_19_contiguous_monotonic_allocations() {
    let mut alloc = BumpAllocator::new();
    let ptr1 = alloc.allocate(64, 8).unwrap() as usize;
    let ptr2 = alloc.allocate(128, 8).unwrap() as usize;
    let ptr3 = alloc.allocate(256, 8).unwrap() as usize;

    assert!(
        ptr2 >= ptr1 + 64,
        "ptr2 (0x{:X}) must not overlap ptr1 (0x{:X} + 64)",
        ptr2,
        ptr1
    );
    assert!(
        ptr3 >= ptr2 + 128,
        "ptr3 (0x{:X}) must not overlap ptr2 (0x{:X} + 128)",
        ptr3,
        ptr2
    );
}

/// `TC-T1-20`: SpinLock Mutual Exclusion.
/// Validates atomic pause spin loop serializes concurrent access across threads.
#[test]
fn t1_sync_20_spinlock_mutual_exclusion() {
    let counter = Arc::new(SpinLock::new(0usize));
    let mut handles = Vec::new();

    const THREADS: usize = 4;
    const INCREMENTS_PER_THREAD: usize = 1000;

    for _ in 0..THREADS {
        let counter_clone = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            for _ in 0..INCREMENTS_PER_THREAD {
                let mut guard = counter_clone.lock();
                *guard += 1;
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let final_val = *counter.lock();
    assert_eq!(
        final_val,
        THREADS * INCREMENTS_PER_THREAD,
        "Protected counter must match exact sum without race conditions"
    );
}

/// `TC-T1-21`: SpinLock RAII Guard Drop Safety.
/// Validates lock automatically releases when guard drops.
#[test]
fn t1_sync_21_spinlock_raii_guard_drop_safety() {
    let lock = SpinLock::new(42);
    assert!(!lock.is_locked());

    {
        let mut guard = lock.lock();
        assert!(lock.is_locked());
        *guard = 100;
        // Guard drops here at end of scope
    }

    assert!(
        !lock.is_locked(),
        "SpinLock must be unlocked after guard drop"
    );

    // Should be able to acquire lock again immediately without deadlock
    let guard2 = lock.try_lock();
    assert!(guard2.is_some(), "Lock acquisition after drop must succeed");
    assert_eq!(*guard2.unwrap(), 100);
}

// ============================================================================
// Tier 2: Boundary & Corner Cases (Stress & Limits)
// ============================================================================

/// `TC-T2-17`: Exact 64 KiB Last-Byte Allocation.
/// Validates allocations totaling exactly 65,536 bytes succeed (used == 65536, free == 0).
#[test]
fn t2_mem_17_exact_64kib_last_byte_allocation() {
    let mut alloc = BumpAllocator::new();
    let ptr = alloc.allocate(65536, 1);
    assert!(
        ptr.is_some(),
        "Exact 64 KiB allocation should succeed on clean heap"
    );
    assert_eq!(alloc.used(), 65536, "Heap used must equal exactly 65536");
    assert_eq!(alloc.free(), 0, "Heap free must equal exactly 0");

    // Any subsequent allocation must fail
    assert!(
        alloc.allocate(1, 1).is_none(),
        "Allocation after exact saturation must return None"
    );
}

/// `TC-T2-18`: Out-of-Memory (OOM) Rejection.
/// Validates allocation request of 65,537 bytes returns None without panic.
#[test]
fn t2_mem_18_out_of_memory_rejection() {
    let mut alloc = BumpAllocator::new();
    // 65537 bytes exceeds 64 KiB total capacity
    let ptr = alloc.allocate(65537, 1);
    assert!(
        ptr.is_none(),
        "Allocation exceeding total capacity must return None"
    );
    assert_eq!(alloc.used(), 0, "Used bytes must remain 0 after OOM");
    assert_eq!(alloc.free(), 65536, "Free bytes must remain 65536 after OOM");
}

/// `TC-T2-19`: Zero-Byte Allocation Request.
/// Validates allocation of size 0 bytes returns None per src/memory.rs:59 without advancing next pointer.
#[test]
fn t2_mem_19_zero_byte_allocation_request() {
    let mut alloc = BumpAllocator::new();
    let initial_used = alloc.used();

    let ptr = alloc.allocate(0, 8);
    assert!(
        ptr.is_none(),
        "Zero-byte allocation must return None per src/memory.rs:59"
    );
    assert_eq!(
        alloc.used(),
        initial_used,
        "Zero-byte allocation must not advance next pointer"
    );
}

/// `TC-T2-20`: Non-Power-of-2 Alignment Rejection.
/// Validates alignment parameter 3, 5, 7, etc. is rejected safely.
#[test]
fn t2_mem_20_non_power_of_2_alignment_rejection() {
    let mut alloc = BumpAllocator::new();
    let non_power_of_2 = [0usize, 3, 5, 6, 7, 9, 10, 15, 33];

    for &align in &non_power_of_2 {
        let ptr = alloc.allocate(16, align);
        assert!(
            ptr.is_none(),
            "Non-power-of-2 alignment {} must be rejected with None",
            align
        );
    }
}

/// `TC-T2-21`: Large Alignment Padding Boundary.
/// Validates 1-byte allocation followed by 4096-byte alignment calculates padding without overflow.
#[test]
fn t2_mem_21_large_alignment_padding_boundary() {
    let mut alloc = BumpAllocator::new();
    alloc.allocate(1, 1); // next = 1

    let ptr = alloc.allocate(4096, 4096);
    assert!(ptr.is_some(), "Large aligned allocation must succeed");
    let addr = ptr.unwrap() as usize;
    assert_eq!(addr % 4096, 0, "Address must be 4096-byte aligned");
    assert_eq!(
        alloc.used(),
        4096 + 4096, // 4096 aligned offset + 4096 size
        "Used must account for both padding and size"
    );
}

/// `TC-T2-22`: Micro-Allocation Saturation.
/// Validates 1,024 single-byte allocations advance pointer monotonically until capacity.
#[test]
fn t2_mem_22_micro_allocation_saturation() {
    let mut alloc = BumpAllocator::new();
    const COUNT: usize = 1024;

    for i in 0..COUNT {
        let ptr = alloc.allocate(1, 1);
        assert!(ptr.is_some(), "Micro-allocation {} must succeed", i);
        assert_eq!(alloc.used(), i + 1, "Used count must track micro-allocations");
    }

    assert_eq!(alloc.used(), COUNT);
    assert_eq!(alloc.free(), 65536 - COUNT);
}
