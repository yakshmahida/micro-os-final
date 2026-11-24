//! Tier 1 & Tier 2 E2E Tests for R3: Workload-Aware MLFQ Scheduling & Task Lifecycle.
//! CS2001 OS Capstone | Student: Yakshendrasinh R Mahida (USN: 25BTRCL288)

use micro_os::task::{Task, TaskClass, TaskState};
use micro_os::scheduler::{MlfqScheduler, ScheduleEvent};

// ============================================================================
// Tier 1: Feature Coverage (Happy Path)
// ============================================================================

/// `TC-T1-12`: Task Control Block Lifecycle.
/// Validates task states `Ready` -> `Running` -> `Terminated`.
#[test]
fn t1_mlfq_12_task_control_block_lifecycle() {
    let task = Task::new(1, "test_task", TaskClass::Interactive, 10);
    assert_eq!(task.id, 1);
    assert_eq!(task.name, "test_task");
    assert_eq!(task.state, TaskState::Ready);
    assert_eq!(task.steps, 10);
    assert_eq!(task.queue, 0);

    // Add to scheduler and run 1 tick
    let mut sched = MlfqScheduler::new();
    sched.add_task(task);
    let event = sched.schedule_once().unwrap();
    assert_eq!(event.task_id, 1);
    assert_eq!(event.remaining_steps, 9);
    assert!(!event.completed);

    // Run until completion (remaining 9 steps)
    for _ in 0..8 {
        sched.schedule_once();
    }
    let last_event = sched.schedule_once().unwrap();
    assert_eq!(last_event.remaining_steps, 0);
    assert!(last_event.completed);
}

/// `TC-T1-13`: 3-Queue MLFQ Quantum Demotion.
/// Validates compute tasks demote Q0 (4ms) -> Q1 (8ms) -> Q2 (32ms).
#[test]
fn t1_mlfq_13_3queue_mlfq_quantum_demotion() {
    let mut sched = MlfqScheduler::new();
    // Long compute task that doesn't yield: 50 steps
    let compute_task = Task::new(0, "compute_heavy", TaskClass::Compute, 50);
    sched.add_task(compute_task);
    sched.boost_interval = 1000; // Delay boost to observe demotions cleanly

    // Ticks 1..4: Runs in Q0 (quantum = 4)
    for _ in 0..4 {
        let event = sched.schedule_once().unwrap();
        assert_eq!(event.queue, 0, "Ticks 1..4 must run in Q0");
    }
    // After 4 ticks in Q0, task expires quantum and demotes to Q1
    let event = sched.schedule_once().unwrap();
    assert_eq!(
        event.queue, 1,
        "Compute task must demote to Q1 after exhausting Q0 quantum (4ms)"
    );

    // Next 7 ticks: runs in Q1 (total 8 ticks in Q1)
    for _ in 0..7 {
        let ev = sched.schedule_once().unwrap();
        assert_eq!(ev.queue, 1, "Must continue running in Q1");
    }

    // After 8 ticks in Q1, task demotes to Q2
    let event = sched.schedule_once().unwrap();
    assert_eq!(
        event.queue, 2,
        "Compute task must demote to Q2 after exhausting Q1 quantum (8ms)"
    );
}

/// `TC-T1-14`: Burst Ratio $\Theta$ Calculation.
/// Validates $\Theta = \frac{t_{\text{exec}}}{t_{\text{exec}} + t_{\text{wait}} + t_{\text{yield}}}$ precision.
#[test]
fn t1_mlfq_14_burst_ratio_theta_calculation() {
    // Interactive task: low execution time, high wait/yield time
    let mut interactive_task = Task::new(1, "shell_io", TaskClass::Interactive, 100);
    interactive_task.exec_time = 10;
    interactive_task.wait_time = 70;
    interactive_task.yield_time = 20;
    let theta_interactive = interactive_task.burst_ratio();
    assert!(
        (theta_interactive - 0.10).abs() < 1e-6,
        "Interactive burst ratio should equal 0.10 (10 / 100)"
    );
    assert!(
        theta_interactive < 0.20,
        "Interactive burst ratio must be strictly less than threshold 0.20"
    );
    assert_eq!(
        interactive_task.calculated_class(),
        TaskClass::Interactive,
        "Calculated class must be Interactive"
    );

    // Compute task: high execution time, low wait/yield time
    let mut compute_task = Task::new(2, "matrix_calc", TaskClass::Compute, 100);
    compute_task.exec_time = 85;
    compute_task.wait_time = 10;
    compute_task.yield_time = 5;
    let theta_compute = compute_task.burst_ratio();
    assert!(
        (theta_compute - 0.85).abs() < 1e-6,
        "Compute burst ratio should equal 0.85 (85 / 100)"
    );
    assert!(
        theta_compute >= 0.20,
        "Compute burst ratio must be greater than or equal to threshold 0.20"
    );
    assert_eq!(
        compute_task.calculated_class(),
        TaskClass::Compute,
        "Calculated class must be Compute"
    );
}

/// `TC-T1-15`: Periodic 50ms Priority Boost.
/// Validates all tasks in Q1/Q2 promoted to Q0 every 5 ticks (50ms).
#[test]
fn t1_mlfq_15_periodic_50ms_priority_boost() {
    let mut sched = MlfqScheduler::new();
    sched.boost_interval = 5; // 50ms at 10ms/tick

    let t1 = Task::new(0, "task1", TaskClass::Compute, 50);
    let t2 = Task::new(1, "task2", TaskClass::Compute, 50);
    sched.add_task(t1);
    sched.add_task(t2);

    // Run until tick 4 (tasks run)
    for _ in 0..4 {
        sched.schedule_once();
    }
    // Now trigger tick 5: priority boost fires!
    let event = sched.schedule_once().unwrap();
    assert!(event.boosted, "Priority boost must fire at tick 5");
    assert_eq!(
        sched.queues[1].len(), 0,
        "Queue Q1 must be completely empty after boost"
    );
    assert_eq!(
        sched.queues[2].len(), 0,
        "Queue Q2 must be completely empty after boost"
    );
}

/// `TC-T1-16`: Deterministic Scheduler Replay.
/// Validates identical task parameters produce byte-identical schedules.
#[test]
fn t1_mlfq_16_deterministic_scheduler_replay() {
    fn run_simulation() -> Vec<ScheduleEvent> {
        let mut sched = MlfqScheduler::new();
        sched.add_task(Task::new(0, "shell", TaskClass::Interactive, 15));
        sched.add_task(Task::new(1, "worker", TaskClass::Compute, 25));
        sched.add_task(Task::new(2, "logger", TaskClass::Interactive, 10));

        let mut trace = Vec::new();
        while let Some(ev) = sched.schedule_once() {
            trace.push(ev);
            if trace.len() >= 40 {
                break;
            }
        }
        trace
    }

    let run1 = run_simulation();
    let run2 = run_simulation();

    assert_eq!(run1.len(), run2.len());
    for (e1, e2) in run1.iter().zip(run2.iter()) {
        assert_eq!(e1.tick, e2.tick);
        assert_eq!(e1.task_id, e2.task_id);
        assert_eq!(e1.queue, e2.queue);
        assert_eq!(e1.remaining_steps, e2.remaining_steps);
        assert_eq!(e1.completed, e2.completed);
    }
}

// ============================================================================
// Tier 2: Boundary & Corner Cases (Stress & Limits)
// ============================================================================

/// `TC-T2-11`: Zero Execution Time Burst Ratio.
/// Validates $t_{\text{exec}} = 0, t_{\text{wait}} = 0$ handles division by zero safely without NaN or panic.
#[test]
fn t2_mlfq_11_zero_execution_time_burst_ratio() {
    let t_interactive = Task::new(0, "fresh_shell", TaskClass::Interactive, 20);
    assert_eq!(t_interactive.exec_time, 0);
    assert_eq!(t_interactive.wait_time, 0);
    assert_eq!(t_interactive.yield_time, 0);

    let ratio_int = t_interactive.burst_ratio();
    assert!(!ratio_int.is_nan(), "Burst ratio must not be NaN");
    assert_eq!(ratio_int, 0.0, "Zero-time Interactive task burst ratio must be 0.0");

    let t_compute = Task::new(1, "fresh_worker", TaskClass::Compute, 20);
    let ratio_comp = t_compute.burst_ratio();
    assert!(!ratio_comp.is_nan(), "Burst ratio must not be NaN");
    assert_eq!(ratio_comp, 0.5, "Zero-time Compute task burst ratio must be 0.5");
}

/// `TC-T2-12`: Continuous Compute Burst ($\ge 1000$ms).
/// Validates heavy compute task running continuously remains in Q2 and classifies as compute.
#[test]
fn t2_mlfq_12_continuous_compute_burst() {
    let mut task = Task::new(0, "heavy_crypto", TaskClass::Compute, 1000);
    task.exec_time = 1000;
    task.wait_time = 0;
    task.yield_time = 0;

    let ratio = task.burst_ratio();
    assert_eq!(ratio, 1.0, "Continuous compute burst must produce theta = 1.0");
    assert!(ratio >= 0.20, "Must be classified as Compute");
    assert_eq!(task.calculated_class(), TaskClass::Compute);
}

/// `TC-T2-13`: Exact Quantum Expiration Boundary.
/// Validates task executes exactly 4ms on Q0 or 8ms on Q1 before demotion.
#[test]
fn t2_mlfq_13_exact_quantum_expiration_boundary() {
    let mut sched = MlfqScheduler::new();
    sched.boost_interval = 100; // No interference from boost
    sched.add_task(Task::new(0, "boundary_task", TaskClass::Compute, 50));

    // Tick 1, 2, 3: Task should stay in Q0
    for tick in 1..=3 {
        let ev = sched.schedule_once().unwrap();
        assert_eq!(
            ev.queue, 0,
            "At tick {}, task must remain in Q0",
            tick
        );
    }

    // Tick 4: Reaches quantum boundary of 4ms
    let ev = sched.schedule_once().unwrap();
    assert_eq!(ev.queue, 0, "At tick 4, task runs on Q0");

    // Tick 5: Must run in Q1
    let ev = sched.schedule_once().unwrap();
    assert_eq!(ev.queue, 1, "At tick 5, task must run in Q1");
}

/// `TC-T2-14`: Exact 50ms Priority Boost Trigger.
/// Validates tick progression at boundaries (ticks 4, 5, 6) triggers boost exclusively at tick 5.
#[test]
fn t2_mlfq_14_exact_50ms_priority_boost_trigger() {
    let mut sched = MlfqScheduler::new();
    sched.boost_interval = 5; // 50ms at 10ms/tick

    let mut t = Task::new(0, "task", TaskClass::Compute, 50);
    t.queue = 2; // Placed in Q2
    sched.queues[2].push_back(t);

    // Ticks 1..4: No boost
    for tick in 1..=4 {
        let ev = sched.schedule_once().unwrap();
        assert!(!ev.boosted, "Tick {} must not be boosted", tick);
    }

    // Tick 5: Priority boost MUST fire
    let ev = sched.schedule_once().unwrap();
    assert!(ev.boosted, "At tick 5, priority boost must fire");

    // Tick 6: Normal execution continues
    let ev = sched.schedule_once().unwrap();
    assert!(!ev.boosted, "Tick 6 must not be boosted");
}

/// `TC-T2-15`: Empty Ready Queue Scheduling Tick.
/// Validates 0 ready tasks in all MLFQ queues returns None safely without panic.
#[test]
fn t2_mlfq_15_empty_ready_queue_scheduling_tick() {
    let mut sched = MlfqScheduler::new();
    assert_eq!(sched.total_tasks(), 0);

    // Advancing tick when queues are completely empty
    for _ in 0..10 {
        let result = sched.schedule_once();
        assert_eq!(
            result, None,
            "Empty scheduler must return None without panicking"
        );
    }
}

/// `TC-T2-16`: Identical Priority Multi-Task Ties.
/// Validates 5 identical tasks added simultaneously round-robin fairly without starvation.
#[test]
fn t2_mlfq_16_identical_priority_multi_task_ties() {
    let mut sched = MlfqScheduler::new();
    sched.boost_interval = 100;

    sched.add_task(Task::new(0, "task_0", TaskClass::Interactive, 20));
    sched.add_task(Task::new(1, "task_1", TaskClass::Interactive, 20));
    sched.add_task(Task::new(2, "task_2", TaskClass::Interactive, 20));
    sched.add_task(Task::new(3, "task_3", TaskClass::Interactive, 20));
    sched.add_task(Task::new(4, "task_4", TaskClass::Interactive, 20));

    let mut execution_counts = [0usize; 5];

    // Run for 20 ticks (4 ticks per task on Q0)
    for _ in 0..20 {
        if let Some(ev) = sched.schedule_once() {
            execution_counts[ev.task_id] += 1;
        }
    }

    // Every task must have received fair CPU time (each got exactly 4 ticks)
    for (i, &count) in execution_counts.iter().enumerate() {
        assert_eq!(
            count, 4,
            "Task {} must receive exactly 4 ticks in round-robin order",
            i
        );
    }
}
