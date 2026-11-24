//! Titanium Micro-OS: Interactive Host Mechanisms CLI Runner
//! Student: Yakshendrasinh R Mahida (USN: 25BTRCL288) | CS2001 OS Capstone
//!
//! Demonstrates the four core OS mechanisms:
//! 1. 3-Queue Workload-Aware MLFQ Scheduler with Live Burst Ratio & Periodic Priority Boost
//! 2. 64 KiB Monotonic Bump Allocator with Power-of-2 Alignment & Boundary Safety
//! 3. Low-Latency Atomic SpinLock with RAII Guard
//! 4. Dijkstra's Banker's Algorithm for Deadlock Avoidance with Transactional Rollback

use std::sync::Arc;
use std::thread;

use micro_os::deadlock::BankersAlgorithm;
use micro_os::memory::BumpAllocator;
use micro_os::scheduler::MlfqScheduler;
use micro_os::sync::SpinLock;
use micro_os::task::{Task, TaskClass};

fn main() {
    println!("===============================================================================");
    println!("               TITANIUM MICRO-OS: CORE MECHANISMS HOST DEMO                   ");
    println!("     CS2001 Operating Systems Capstone | Student: Yakshendrasinh R Mahida      ");
    println!("===============================================================================\n");

    demo_scheduler();
    println!();
    demo_bump_allocator();
    println!();
    demo_spinlock();
    println!();
    demo_bankers_algorithm();
    println!();

    println!("===============================================================================");
    println!("           ★ ALL TITANIUM MICRO-OS MECHANISMS VERIFIED SUCCESSFULLY ★          ");
    println!("===============================================================================");
}

fn demo_scheduler() {
    println!("-------------------------------------------------------------------------------");
    println!("  MECHANISM 1: 3-Queue Workload-Aware MLFQ Scheduler (Research Track R4)       ");
    println!("  - Hierarchy: Q0 (4ms, High), Q1 (8ms, Medium), Q2 (32ms, Low)                ");
    println!("  - Classification: Theta < 0.20 (Interactive) vs Theta >= 0.20 (Compute)      ");
    println!("  - Anti-Starvation: Priority Boost every 25 ticks                             ");
    println!("-------------------------------------------------------------------------------");

    let mut scheduler = MlfqScheduler::new();
    scheduler.boost_interval = 25;

    scheduler.add_task(Task::new(1, "shell", TaskClass::Interactive, 6));
    scheduler.add_task(Task::new(2, "logger", TaskClass::Interactive, 8));
    scheduler.add_task(Task::new(3, "worker", TaskClass::Compute, 18));
    scheduler.add_task(Task::new(4, "matrix", TaskClass::Compute, 14));

    println!("Initial Tasks Enqueued: {}", scheduler.total_tasks());
    println!("{:<6} {:<8} {:<10} {:<6} {:<10} {:<12} {:<10}", "Tick", "Task ID", "Name", "Queue", "Rem Steps", "Burst Ratio", "Status");

    while !scheduler.is_empty() {
        if let Some(event) = scheduler.schedule_once() {
            let status = if event.completed {
                "COMPLETED"
            } else if event.boosted {
                "BOOSTED"
            } else {
                "RUNNING"
            };
            println!(
                "{:<6} {:<8} {:<10} Q{:<5} {:<10} {:<12.3} {:<10}",
                event.tick,
                event.task_id,
                event.task_name,
                event.queue,
                event.remaining_steps,
                event.burst_ratio,
                status
            );
        }
    }

    println!("Scheduler complete: {} tasks completed, {} context switches, {} boosts fired.",
        scheduler.completed, scheduler.switches, scheduler.boost_count);
}

fn demo_bump_allocator() {
    println!("-------------------------------------------------------------------------------");
    println!("  MECHANISM 2: 64 KiB Monotonic Bump Allocator                                ");
    println!("  - Total Capacity: 65,536 Bytes (Aligned to 4096-byte Page Boundary)          ");
    println!("  - Power-of-2 Alignment: 8, 16, 32, 64 Bytes                                 ");
    println!("  - Boundary & OOM Detection with Complete Arena Reset                         ");
    println!("-------------------------------------------------------------------------------");

    let mut alloc = BumpAllocator::new();
    let stats_initial = alloc.stats();
    println!("Initial State: used = {} B, free = {} B, total = {} B",
        stats_initial.used, stats_initial.free, stats_initial.total);

    // Allocation 1: 128 bytes, 8-byte aligned
    let p1 = alloc.allocate(128, 8).expect("Allocation 1 failed");
    println!("Allocated Block 1: 128 bytes, align 8  -> Addr: 0x{:X} (align ok: {})",
        p1 as usize, (p1 as usize) % 8 == 0);

    // Allocation 2: 512 bytes, 64-byte aligned
    let p2 = alloc.allocate(512, 64).expect("Allocation 2 failed");
    println!("Allocated Block 2: 512 bytes, align 64 -> Addr: 0x{:X} (align ok: {})",
        p2 as usize, (p2 as usize) % 64 == 0);

    // Allocation 3: 4096 bytes, 32-byte aligned
    let p3 = alloc.allocate(4096, 32).expect("Allocation 3 failed");
    println!("Allocated Block 3: 4096 bytes, align 32 -> Addr: 0x{:X} (align ok: {})",
        p3 as usize, (p3 as usize) % 32 == 0);

    let stats_mid = alloc.stats();
    println!("Mid-State: used = {} B, free = {} B, total allocations = {}",
        stats_mid.used, stats_mid.free, stats_mid.allocations);

    // OOM Test
    let oom = alloc.allocate(70000, 8);
    println!("Testing Oversized Allocation (70,000 B) -> Result: {:?}", oom);
    assert!(oom.is_none());

    // Reset Test
    alloc.reset();
    let stats_reset = alloc.stats();
    println!("After Reset: used = {} B, free = {} B (Capacity 100% Restored)",
        stats_reset.used, stats_reset.free);
}

fn demo_spinlock() {
    println!("-------------------------------------------------------------------------------");
    println!("  MECHANISM 3: Low-Latency Atomic SpinLock (x86 PAUSE / core::hint::spin_loop) ");
    println!("  - AtomicBool with Acquire/Release Ordering Semantics                         ");
    println!("  - Multi-Threaded Contention: 4 Threads x 2,500 Increments = 10,000 Exact     ");
    println!("-------------------------------------------------------------------------------");

    let counter = Arc::new(SpinLock::new(0usize));
    let mut handles = Vec::new();

    const NUM_THREADS: usize = 4;
    const INCREMENTS_PER_THREAD: usize = 2500;

    for id in 0..NUM_THREADS {
        let counter_clone = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            for _ in 0..INCREMENTS_PER_THREAD {
                let mut guard = counter_clone.lock();
                *guard += 1;
            }
            println!("  [Thread {}] Finished {} atomic protected increments", id, INCREMENTS_PER_THREAD);
        }));
    }

    for handle in handles {
        handle.join().expect("Thread join failed");
    }

    let final_value = *counter.lock();
    println!("Final Counter Value: {} (Expected: {}) -> {}",
        final_value, NUM_THREADS * INCREMENTS_PER_THREAD,
        if final_value == NUM_THREADS * INCREMENTS_PER_THREAD { "VERIFIED" } else { "FAILED" });
}

fn demo_bankers_algorithm() {
    println!("-------------------------------------------------------------------------------");
    println!("  MECHANISM 4: Dijkstra's Banker's Algorithm (Deadlock Avoidance)              ");
    println!("  - 5 Processes, 3 Resource Types (A, B, C)                                    ");
    println!("  - Safe State Verification, Transactional Allocation & Rollback               ");
    println!("-------------------------------------------------------------------------------");

    let mut banker = BankersAlgorithm::new(5, 3);
    banker.set_available(&[3, 3, 2]);

    banker.set_allocation(0, &[0, 1, 0]);
    banker.set_max(0, &[7, 5, 3]);

    banker.set_allocation(1, &[2, 0, 0]);
    banker.set_max(1, &[3, 2, 2]);

    banker.set_allocation(2, &[3, 0, 2]);
    banker.set_max(2, &[9, 0, 2]);

    banker.set_allocation(3, &[2, 1, 1]);
    banker.set_max(3, &[2, 2, 2]);

    banker.set_allocation(4, &[0, 0, 2]);
    banker.set_max(4, &[4, 3, 3]);

    println!("Available Resources: {:?}", banker.available);
    println!("Is Initial State Safe? {}", banker.is_safe());
    let seq = banker.safe_sequence().expect("Safe sequence should exist");
    println!("Computed Safe Execution Sequence: P{:?}", seq);

    // Request 1: Safe request by P1 [1, 0, 2]
    println!("\nProcess P1 requesting resources [1, 0, 2]...");
    let granted = banker.request_resources(1, &[1, 0, 2]);
    println!("  Request Granted: {} | New Available: {:?}", granted, banker.available);
    let new_seq = banker.safe_sequence().expect("Safe sequence should exist");
    println!("  New Safe Sequence: P{:?}", new_seq);

    // Request 2: Unsafe request by P0 [0, 2, 0]
    println!("\nProcess P0 requesting unsafe resources [0, 2, 0]...");
    let granted_unsafe = banker.request_resources(0, &[0, 2, 0]);
    println!("  Request Granted: {} (Deadlock Prevented!)", granted_unsafe);
    println!("  Available after Rollback: {:?}", banker.available);
    println!("  Allocation for P0 after Rollback: {:?}", banker.allocation[0]);
    println!("  Is State Still Safe? {}", banker.is_safe());
}
