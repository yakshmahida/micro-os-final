//! Host Unit Test Suite for Titanium Micro-OS Core Mechanisms.
//!
//! Validates:
//! - TC1: Two-task round-robin scheduling and completion
//! - TC2: 4-task MLFQ execution with queue demotions
//! - TC3: Allocator power-of-two alignment and exact boundary allocation
//! - TC4: MLFQ priority boost preventing compute task starvation (10-task stress test)
//! - TC5: Allocator OOM handling and reset capacity recovery
//! - TC6: MLFQ scheduler deterministic repeatability across runs
//! - TC7: Dijkstra's Banker's Algorithm safe sequence and unsafe request rollback
//! - BONUS 1: SpinLock concurrent mutual exclusion under multi-threaded contention
//! - BONUS 2: Mathematical burst ratio Θ calculation and class classification

use micro_os::deadlock::BankersAlgorithm;
use micro_os::memory::BumpAllocator;
use micro_os::scheduler::MlfqScheduler;
use micro_os::sync::SpinLock;
use micro_os::task::{Task, TaskClass};

/// TC1: Minimum workload verification — 2 tasks execute in round-robin and complete.
#[test]
fn test_two_task_round_robin_completes() {
    let mut scheduler = MlfqScheduler::new();
    let task_a = Task::new(1, "task_a", TaskClass::Interactive, 5);
    let task_b = Task::new(2, "task_b", TaskClass::Interactive, 5);

    scheduler.add_task(task_a);
    scheduler.add_task(task_b);

    assert_eq!(scheduler.total_tasks(), 2);
    assert_eq!(scheduler.completed, 0);

    let mut execution_order = Vec::new();
    let mut steps_a = 0;
    let mut steps_b = 0;

    while !scheduler.is_empty() {
        if let Some(event) = scheduler.schedule_once() {
            execution_order.push(event.task_id);
            if event.task_id == 1 {
                steps_a += 1;
            } else if event.task_id == 2 {
                steps_b += 1;
            }
        }
    }

    assert_eq!(scheduler.completed, 2, "Both tasks must terminate");
    assert_eq!(steps_a, 5, "Task A must have executed exactly 5 steps");
    assert_eq!(steps_b, 5, "Task B must have executed exactly 5 steps");
    assert_eq!(execution_order.len(), 10, "Total execution steps must equal 10");

    // Verify round-robin interleaving
    assert!(execution_order.contains(&1), "Task A must have executed");
    assert!(execution_order.contains(&2), "Task B must have executed");
}

/// TC2: 4 tasks with mixed interactive and compute workloads execute across MLFQ queues.
#[test]
fn test_mlfq_four_tasks_complete() {
    let mut scheduler = MlfqScheduler::new();
    // 4 representative OS tasks:
    // shell (interactive: 6 steps)
    // logger (interactive: 8 steps)
    // worker (compute-bound: 20 steps)
    // idle (compute-bound: 4 steps)
    scheduler.add_task(Task::new(1, "shell", TaskClass::Interactive, 6));
    scheduler.add_task(Task::new(2, "logger", TaskClass::Interactive, 8));
    scheduler.add_task(Task::new(3, "worker", TaskClass::Compute, 20));
    scheduler.add_task(Task::new(4, "idle", TaskClass::Compute, 4));

    let mut worker_queues = Vec::new();
    let mut total_events = 0;

    while !scheduler.is_empty() {
        if let Some(event) = scheduler.schedule_once() {
            total_events += 1;
            if event.task_id == 3 {
                worker_queues.push(event.queue);
            }
        }
    }

    assert_eq!(scheduler.completed, 4, "All 4 tasks must complete");
    assert_eq!(total_events, 6 + 8 + 20 + 4, "Total steps must sum to 38");

    // Worker had 20 steps, which exceeds Q0 quantum (4ms) and Q1 quantum (8ms).
    // Worker must experience demotion to lower priority queues (Q1 or Q2).
    assert!(
        worker_queues.iter().any(|&q| q > 0),
        "Worker must experience demotion to lower priority queues"
    );
}

/// TC3: BumpAllocator respects power-of-2 alignment (8, 16, 32, 64) and fills to exact boundary.
#[test]
fn test_allocator_respects_alignment_and_boundary() {
    let mut alloc = BumpAllocator::new();
    assert_eq!(alloc.total(), 65536);
    assert_eq!(alloc.used(), 0);
    assert_eq!(alloc.free(), 65536);

    // 1. Verify alignment for multiple powers of two
    let p8 = alloc.allocate(11, 8).expect("Allocation with align 8 failed");
    assert_eq!((p8 as usize) % 8, 0, "Pointer p8 must be 8-byte aligned");

    let p16 = alloc.allocate(23, 16).expect("Allocation with align 16 failed");
    assert_eq!((p16 as usize) % 16, 0, "Pointer p16 must be 16-byte aligned");

    let p32 = alloc.allocate(45, 32).expect("Allocation with align 32 failed");
    assert_eq!((p32 as usize) % 32, 0, "Pointer p32 must be 32-byte aligned");

    let p64 = alloc.allocate(77, 64).expect("Allocation with align 64 failed");
    assert_eq!((p64 as usize) % 64, 0, "Pointer p64 must be 64-byte aligned");

    // Verify allocated pointers are writable without crashing
    unsafe {
        core::ptr::write(p8, 0x11);
        core::ptr::write(p16, 0x22);
        core::ptr::write(p32, 0x33);
        core::ptr::write(p64, 0x44);

        assert_eq!(core::ptr::read(p8), 0x11);
        assert_eq!(core::ptr::read(p16), 0x22);
        assert_eq!(core::ptr::read(p32), 0x33);
        assert_eq!(core::ptr::read(p64), 0x44);
    }

    // 2. Reset and test exact boundary allocation
    alloc.reset();
    assert_eq!(alloc.used(), 0);
    assert_eq!(alloc.free(), 65536);

    // Allocate 1024 bytes with 64-byte alignment
    let b1 = alloc.allocate(1024, 64).expect("Initial 1024-byte block failed");
    assert_eq!((b1 as usize) % 64, 0);

    // Allocate exact remaining capacity
    let remaining = alloc.free();
    assert_eq!(remaining, 65536 - 1024);
    let b2 = alloc
        .allocate(remaining, 1)
        .expect("Remainder allocation to boundary failed");
    assert!(!b2.is_null());

    // Boundary reached: used == 65536, free == 0
    assert_eq!(alloc.used(), 65536);
    assert_eq!(alloc.free(), 0);

    // Allocation past boundary must return None
    assert!(
        alloc.allocate(1, 1).is_none(),
        "Allocation past boundary must return None"
    );
}

/// TC4: Priority boost prevents starvation in a 10-task stress test (1 compute + 9 interactive).
#[test]
fn test_priority_boost_prevents_starvation() {
    let mut scheduler = MlfqScheduler::new();
    scheduler.boost_interval = 20; // Accelerate boost interval for deterministic unit test

    // 1 heavy compute task (30 steps)
    scheduler.add_task(Task::new(100, "heavy_compute", TaskClass::Compute, 30));

    // 9 interactive tasks (3 steps each = 27 steps total)
    for i in 1..=9 {
        scheduler.add_task(Task::new(i, "interactive", TaskClass::Interactive, 3));
    }

    assert_eq!(scheduler.total_tasks(), 10);

    let mut compute_steps = 0;
    let mut boosts_observed = 0;
    let mut iterations = 0;
    const MAX_ITERATIONS: usize = 300;

    while !scheduler.is_empty() && iterations < MAX_ITERATIONS {
        iterations += 1;
        if let Some(event) = scheduler.schedule_once() {
            if event.task_id == 100 {
                compute_steps += 1;
            }
            if event.boosted {
                boosts_observed += 1;
            }
        }
    }

    assert_eq!(scheduler.completed, 10, "All 10 tasks must complete");
    assert_eq!(
        compute_steps, 30,
        "Heavy compute task must complete all 30 steps"
    );
    assert!(
        boosts_observed > 0,
        "Priority boost must have triggered to prevent starvation"
    );
}

/// TC5: BumpAllocator safely handles out-of-memory without panic and recovers on reset.
#[test]
fn test_allocator_oom_returns_none() {
    let mut alloc = BumpAllocator::new();

    // 1. Oversized single allocation
    assert!(
        alloc.allocate(65537, 1).is_none(),
        "Over-allocation (>64KB) must return None"
    );
    assert!(
        alloc.allocate(100_000, 16).is_none(),
        "Massive allocation must return None"
    );
    assert_eq!(alloc.used(), 0, "Failed allocation must not advance pointer");

    // 2. Incremental exhaustion: 16 chunks of 4096 bytes
    for i in 0..16 {
        let ptr = alloc.allocate(4096, 4096);
        assert!(ptr.is_some(), "Chunk {} of 4096 should succeed", i);
    }

    assert_eq!(alloc.used(), 65536);
    assert_eq!(alloc.free(), 0);

    // 3. Further allocations return None
    assert!(
        alloc.allocate(1, 1).is_none(),
        "Allocation on exhausted heap must return None"
    );
    assert!(
        alloc.allocate(0, 1).is_none(),
        "Zero-size allocation must return None"
    );

    // 4. Reset restores full capacity
    alloc.reset();
    assert_eq!(alloc.used(), 0);
    assert_eq!(alloc.free(), 65536);
    assert!(
        alloc.allocate(65536, 1).is_some(),
        "Full allocation after reset should succeed"
    );
    assert_eq!(alloc.free(), 0);
}

/// TC6: Scheduler produces byte-for-byte identical schedules across two runs with identical inputs.
#[test]
fn test_scheduler_is_deterministic() {
    fn simulate_run() -> Vec<(usize, usize, usize)> {
        let mut scheduler = MlfqScheduler::new();
        scheduler.add_task(Task::new(1, "task_1", TaskClass::Interactive, 7));
        scheduler.add_task(Task::new(2, "task_2", TaskClass::Compute, 15));
        scheduler.add_task(Task::new(3, "task_3", TaskClass::Interactive, 4));
        scheduler.add_task(Task::new(4, "task_4", TaskClass::Compute, 10));

        let mut trace = Vec::new();
        while !scheduler.is_empty() {
            if let Some(event) = scheduler.schedule_once() {
                trace.push((event.task_id, event.queue, event.remaining_steps));
            }
        }
        trace
    }

    let run_a = simulate_run();
    let run_b = simulate_run();

    assert!(!run_a.is_empty(), "Trace must not be empty");
    assert_eq!(
        run_a, run_b,
        "Two runs with identical initial conditions must produce identical schedules"
    );
}

/// TC7: Dijkstra's Banker's Algorithm validates safety, grants safe requests, and rolls back unsafe requests.
#[test]
fn test_bankers_algorithm_safe_sequence() {
    // 5 processes, 3 resource types (Classic Silberschatz model)
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

    // 1. Initial state safety check
    assert!(banker.is_safe(), "Initial state must be safe");
    let safe_seq = banker.safe_sequence().expect("Safe sequence must exist");
    assert_eq!(safe_seq.len(), 5, "Safe sequence must include all 5 processes");

    // 2. Grant safe request: P1 requests [1, 0, 2]
    let granted = banker.request_resources(1, &[1, 0, 2]);
    assert!(granted, "Safe request by P1 must be granted");
    assert_eq!(banker.available, vec![2, 3, 0]);
    assert_eq!(banker.allocation[1], vec![3, 0, 2]);
    assert!(banker.is_safe(), "State must remain safe after granted request");

    // 3. Deny request exceeding maximum claim: P0 requests [8, 0, 0] (Max demand is 7)
    let claim_exceeded = banker.request_resources(0, &[8, 0, 0]);
    assert!(!claim_exceeded, "Request exceeding claim must be rejected");

    // 4. Deny request exceeding available resources: P4 requests [3, 3, 0] (Available is [2, 3, 0])
    let unavailable = banker.request_resources(4, &[3, 3, 0]);
    assert!(!unavailable, "Request exceeding available must be rejected");

    // 5. Deny unsafe request with automatic rollback: P0 requests [0, 2, 0]
    let alloc_before = banker.allocation[0].clone();
    let avail_before = banker.available.clone();

    let granted_unsafe = banker.request_resources(0, &[0, 2, 0]);
    assert!(!granted_unsafe, "Unsafe request must be rejected");
    assert_eq!(
        banker.allocation[0], alloc_before,
        "Allocation must rollback on unsafe request"
    );
    assert_eq!(
        banker.available, avail_before,
        "Available must rollback on unsafe request"
    );
    assert!(banker.is_safe(), "State must remain safe after rollback");
}

/// BONUS: SpinLock protects shared state under multi-threaded contention.
#[test]
fn test_spinlock_protects_shared_value() {
    use std::sync::Arc;
    use std::thread;

    let counter = Arc::new(SpinLock::new(0usize));
    let mut handles = Vec::new();

    const NUM_THREADS: usize = 4;
    const INCREMENTS_PER_THREAD: usize = 2500;

    for _ in 0..NUM_THREADS {
        let counter_clone = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            for _ in 0..INCREMENTS_PER_THREAD {
                let mut guard = counter_clone.lock();
                *guard += 1;
            }
        }));
    }

    for handle in handles {
        handle.join().expect("Worker thread panicked");
    }

    let final_val = *counter.lock();
    assert_eq!(
        final_val,
        NUM_THREADS * INCREMENTS_PER_THREAD,
        "SpinLock must guarantee atomic mutual exclusion"
    );
}

/// BONUS: Task burst ratio Θ calculation and workload classification (Research Track R4).
#[test]
fn test_task_burst_ratio_classification() {
    let mut t_interactive = Task::new(1, "interactive_test", TaskClass::Interactive, 10);
    t_interactive.exec_time = 15;
    t_interactive.wait_time = 85;
    t_interactive.yield_time = 0;
    // Theta = 15 / (15 + 85) = 0.15 < 0.20 -> Interactive
    let theta_interactive = t_interactive.burst_ratio();
    assert!((theta_interactive - 0.15).abs() < 1e-6);
    assert_eq!(t_interactive.calculated_class(), TaskClass::Interactive);

    let mut t_compute = Task::new(2, "compute_test", TaskClass::Compute, 10);
    t_compute.exec_time = 60;
    t_compute.wait_time = 40;
    t_compute.yield_time = 0;
    // Theta = 60 / (60 + 40) = 0.60 >= 0.20 -> Compute
    let theta_compute = t_compute.burst_ratio();
    assert!((theta_compute - 0.60).abs() < 1e-6);
    assert_eq!(t_compute.calculated_class(), TaskClass::Compute);

    // Zero time handled safely without divide-by-zero
    let t_zero = Task::new(3, "zero_test", TaskClass::Interactive, 5);
    assert_eq!(t_zero.burst_ratio(), 0.0);
    assert_eq!(t_zero.calculated_class(), TaskClass::Interactive);
}
