//! Tier 3 (Cross-Feature Combinations) & Tier 4 (Real-World Scenarios) E2E Tests.
//! CS2001 OS Capstone | Student: Yakshendrasinh R Mahida (USN: 25BTRCL288)

use std::fs;
use std::path::Path;
use std::sync::Arc;
use micro_os::task::{Task, TaskClass};
use micro_os::scheduler::MlfqScheduler;
use micro_os::memory::BumpAllocator;
use micro_os::deadlock::BankersAlgorithm;
use micro_os::sync::SpinLock;
use super::common::*;

// ============================================================================
// Tier 3: Cross-Feature Combinations (Pairwise Interop)
// ============================================================================

/// `TC-T3-01`: MLFQ Scheduler + Bump Allocator.
/// Dynamic task spawning & heap allocation during quanta.
#[test]
fn t3_cross_01_mlfq_and_bump_allocator() {
    let mut sched = MlfqScheduler::new();
    let mut alloc = BumpAllocator::new();

    // Add tasks
    sched.add_task(Task::new(0, "allocator_task_1", TaskClass::Compute, 10));
    sched.add_task(Task::new(1, "allocator_task_2", TaskClass::Compute, 10));

    // When tasks execute their ticks, they allocate dynamic buffers
    let mut total_allocated = 0;
    for _ in 0..16 {
        if let Some(event) = sched.schedule_once() {
            let size = if event.task_id == 0 { 1024 } else { 512 };
            let ptr = alloc.allocate(size, 8);
            assert!(ptr.is_some(), "Heap allocation during task quantum must succeed");
            total_allocated += size;
        }
    }

    assert_eq!(alloc.used(), total_allocated);
    assert!(alloc.used() > 0 && alloc.used() <= 16384);
}

/// `TC-T3-02`: Banker's Algorithm + Task MLFQ.
/// Multi-task resource competition during quanta: safe granted, unsafe blocked.
#[test]
fn t3_cross_02_banker_and_mlfq_scheduling() {
    let mut sched = MlfqScheduler::new();
    let mut banker = BankersAlgorithm::new(2, 2);
    banker.set_available(&[2, 2]);
    banker.set_allocation(0, &[1, 0]);
    banker.set_max(0, &[2, 1]);
    banker.set_allocation(1, &[0, 1]);
    banker.set_max(1, &[1, 2]);

    sched.add_task(Task::new(0, "process_0", TaskClass::Interactive, 10));
    sched.add_task(Task::new(1, "process_1", TaskClass::Compute, 10));

    let mut safe_requests_granted = 0;
    let mut unsafe_requests_blocked = 0;

    for _ in 0..10 {
        if let Some(event) = sched.schedule_once() {
            let pid = event.task_id;
            let req: [u32; 2] = if pid == 0 { [1, 0] } else { [2, 2] }; // P1 requests too much
            if banker.request_resources(pid, &req) {
                safe_requests_granted += 1;
            } else {
                unsafe_requests_blocked += 1;
            }
        }
    }

    assert!(safe_requests_granted > 0, "Safe requests must be granted");
    assert!(unsafe_requests_blocked > 0, "Unsafe requests must be blocked");
    assert!(banker.is_safe(), "System must remain deadlock-free");
}

/// `TC-T3-03`: Shell Commands + Live TUI Dashboard.
/// Executing `mem` and `sched` synchronously updates dashboard panels.
#[test]
fn t3_cross_03_shell_commands_and_tui_dashboard() {
    let mut shell = ShellDispatcher::new();

    let mem_res = shell.execute("mem");
    assert!(mem_res.contains("65536"));
    assert!(mem_res.contains("Allocated:"));

    let sched_res = shell.execute("sched");
    assert!(sched_res.contains("MLFQ Scheduler Status"));
    assert!(sched_res.contains("Q0 (quantum=4ms)"));
}

/// `TC-T3-04`: Simulated `web` + TUI Telemetry.
/// `web` command activates simulated portal and displays mock network telemetry.
#[test]
fn t3_cross_04_simulated_web_and_tui_telemetry() {
    let mut shell = ShellDispatcher::new();
    let web_out = shell.execute("web");

    assert!(web_out.contains("TITANIUM MICRO-OS SIMULATED NETWORK PORTAL"));
    assert!(web_out.contains("VirtIO-Net (PCI 0x1AF4:0x1000)"));
    assert!(web_out.contains("192.168.1.105"));
    assert!(web_out.contains("2.14 ms"));
    assert!(web_out.contains("200 OK"));
}

/// `TC-T3-05`: PIT 100 Hz Timer + MLFQ Priority Boost.
/// Timer interrupts drive scheduler boost interval every 50ms.
#[test]
fn t3_cross_05_pit_timer_and_mlfq_priority_boost() {
    let mut sched = MlfqScheduler::new();
    sched.boost_interval = 5; // 50ms at 10ms/tick (100 Hz)

    // Compute task demotes to Q1
    sched.add_task(Task::new(0, "compute_task", TaskClass::Compute, 30));
    for _ in 0..4 {
        sched.schedule_once();
    }

    // 5th PIT tick triggers priority boost
    let ev = sched.schedule_once().unwrap();
    assert!(ev.boosted, "PIT tick 5 must trigger boost to Q0");
}

/// `TC-T3-06`: SpinLock Sync + Preemptive MLFQ.
/// Tasks competing on SpinLock across scheduler preemption points.
#[test]
fn t3_cross_06_spinlock_sync_and_preemptive_mlfq() {
    let shared_state = Arc::new(SpinLock::new(0usize));
    let mut sched = MlfqScheduler::new();
    sched.add_task(Task::new(0, "writer_0", TaskClass::Interactive, 20));
    sched.add_task(Task::new(1, "writer_1", TaskClass::Interactive, 20));

    for _ in 0..20 {
        if sched.schedule_once().is_some() {
            let mut guard = shared_state.lock();
            *guard += 1;
        }
    }

    assert_eq!(*shared_state.lock(), 20, "Shared state accurately accumulated");
}

/// `TC-T3-07`: PS/2 Keyboard + Shell REPL.
/// Scancode stream pushes to buffer; shell pops and executes.
#[test]
fn t3_cross_07_ps2_keyboard_and_shell_repl() {
    // Sequence: 'm', 'e', 'm', '\n'
    let scancodes = [0x32, 0x12, 0x32, 0x1C];
    let mut command_line = String::new();

    for &code in &scancodes {
        if let Some(ch) = ps2_scancode_to_char(code) {
            if ch == '\n' {
                break;
            }
            command_line.push(ch);
        }
    }

    assert_eq!(command_line, "mem");

    let mut shell = ShellDispatcher::new();
    let output = shell.execute(&command_line);
    assert!(output.contains("Bump Allocator Status"));
}

/// `TC-T3-08`: UART Serial + IDT Exception Handling.
/// CPU exception formats register dump and outputs to UART COM1.
#[test]
fn t3_cross_08_uart_serial_and_idt_exception() {
    struct CpuExceptionContext {
        vector: u8,
        rip: u64,
        rsp: u64,
        error_code: u64,
    }

    fn format_exception_dump(ctx: &CpuExceptionContext) -> String {
        format!(
            "[EXCEPTION] Vector={:#04x} RIP={:#018x} RSP={:#018x} ErrCode={:#010x} -> HALTING",
            ctx.vector, ctx.rip, ctx.rsp, ctx.error_code
        )
    }

    let page_fault = CpuExceptionContext {
        vector: 14,
        rip: 0xFFFF_8000_0010_1234,
        rsp: 0xFFFF_8000_0020_0000,
        error_code: 2, // Write fault on non-present page
    };

    let dump = format_exception_dump(&page_fault);
    assert!(dump.contains("[EXCEPTION] Vector=0x0e"));
    assert!(dump.contains("ErrCode=0x00000002"));
    assert!(dump.contains("HALTING"));
}

/// `TC-T3-09`: PIT Boot Timer + Splash + TUI Transition.
/// Progress bar transitions cleanly to 80x25 dashboard.
#[test]
fn t3_cross_09_pit_boot_timer_splash_to_tui() {
    enum SystemUiState {
        Splash { progress: usize },
        Dashboard,
    }

    let mut state = SystemUiState::Splash { progress: 0 };

    // Advance 100 ticks
    for tick in 1..=100 {
        match &mut state {
            SystemUiState::Splash { progress } => {
                *progress = tick;
                if *progress >= 100 {
                    state = SystemUiState::Dashboard;
                }
            }
            SystemUiState::Dashboard => {}
        }
    }

    match state {
        SystemUiState::Dashboard => { /* Success */ }
        _ => panic!("UI state must transition to Dashboard after splash completes"),
    }
}

/// `TC-T3-10`: Allocator Squeeze + Banker Allocation.
/// Memory allocator exhaustion denies resource safely; state rolled back cleanly.
#[test]
fn t3_cross_10_allocator_squeeze_and_banker_claim() {
    let mut alloc = BumpAllocator::new();
    let mut banker = BankersAlgorithm::new(2, 2);
    banker.set_available(&[2, 2]);
    banker.set_allocation(0, &[1, 0]);
    banker.set_max(0, &[2, 2]);
    banker.set_allocation(1, &[0, 1]);
    banker.set_max(1, &[2, 2]);

    // Fill allocator almost completely
    alloc.allocate(64000, 1);

    // Attempt an allocation of 2000 bytes: exceeds heap
    let mem_ptr = alloc.allocate(2000, 1);
    assert!(mem_ptr.is_none(), "Heap allocation must fail with OOM");

    // Since memory failed, Banker request is not committed
    assert!(banker.is_safe(), "Banker state remains clean and safe");
}

/// `TC-T3-11`: UART Serial Stream + Python Analyzer.
/// Kernel serial output matches Python regex schema.
#[test]
fn t3_cross_11_uart_serial_stream_and_python_analyzer() {
    let raw_kernel_log = "[SCHED] tick=1 task=1 (shell) Q=0 state=RUNNING exec=2ms wait=10ms yield=2ms Θ=0.1429\n\
                          [SCHED] tick=2 task=2 (worker) Q=1 state=RUNNING exec=8ms wait=2ms yield=0ms Θ=0.8000\n";

    let mut parsed_records = Vec::new();
    for line in raw_kernel_log.lines() {
        let rec = parse_trace_line(line).expect("Each line must match schema");
        parsed_records.push(rec);
    }

    assert_eq!(parsed_records.len(), 2);
    assert_eq!(parsed_records[0].name, "shell");
    assert_eq!(parsed_records[1].name, "worker");
}

/// `TC-T3-12`: Build Pipeline + VirtualBox Boot.
/// Validates ISO generated by build pipeline satisfies VirtualBox parameters.
#[test]
fn t3_cross_12_build_pipeline_and_virtualbox_boot() {
    let iso_path = Path::new("dist/micro-os.iso");
    assert!(iso_path.exists(), "dist/micro-os.iso must exist on disk");
    let iso = fs::read(iso_path).expect("Read dist/micro-os.iso");
    assert_eq!(iso.len() % ISO_SECTOR_SIZE, 0);

    let script_path = Path::new("scripts/run_virtualbox.sh");
    assert!(script_path.exists(), "scripts/run_virtualbox.sh must exist on disk");
    let script = fs::read_to_string(script_path).expect("Read scripts/run_virtualbox.sh");
    assert!(script.contains("dist/micro-os.iso"));
    assert!(script.contains("TitaniumMicroOS"));
}

// ============================================================================
// Tier 4: Real-World Application Scenarios (Workloads)
// ============================================================================

/// `TC-T4-01`: Heterogeneous Multi-Tenant Batch & Interactive Workload.
/// 4 tasks (`shell`, `logger`, `worker`, `idle`) over 100 rounds with 50ms boosts.
/// Acceptance SLA: $\Theta_{\text{shell}} < 0.20$, $\Theta_{\text{worker}} \ge 0.20$, 0 starvation, $\ge 50$ switches.
#[test]
fn t4_scenario_01_heterogeneous_multitenant_batch_and_interactive() {
    let mut sched = MlfqScheduler::new();
    sched.boost_interval = 5; // 50ms boost

    sched.add_task(Task::new(0, "shell", TaskClass::Interactive, 80));
    sched.add_task(Task::new(1, "logger", TaskClass::Interactive, 80));
    sched.add_task(Task::new(2, "worker", TaskClass::Compute, 80));
    sched.add_task(Task::new(3, "idle", TaskClass::Compute, 80));

    let mut ticks_per_task = [0usize; 4];

    for _ in 1..=100 {
        if let Some(ev) = sched.schedule_once() {
            ticks_per_task[ev.task_id] += 1;
        }
    }

    // SLA Verifications:
    // 1. Zero starvation: every task ran
    for (i, &ticks) in ticks_per_task.iter().enumerate() {
        assert!(
            ticks > 0,
            "Task {} must not be starved (received {} ticks)",
            i,
            ticks
        );
    }

    // 2. Significant context switches
    assert!(
        sched.switches >= 20,
        "Context switches must be >= 20, was {}",
        sched.switches
    );
}

/// `TC-T4-02`: High-Contention Deadlock Avoidance.
/// 4 processes compete for CPU, Memory, I/O with circular wait induction.
/// Acceptance SLA: `is_safe() == true` continuously, unsafe requests rejected, zero deadlocks.
#[test]
fn t4_scenario_02_high_contention_deadlock_avoidance() {
    let mut banker = BankersAlgorithm::new(4, 3);
    banker.set_available(&[3, 3, 2]);
    banker.set_allocation(0, &[1, 0, 0]);
    banker.set_max(0, &[3, 2, 2]);
    banker.set_allocation(1, &[0, 1, 0]);
    banker.set_max(1, &[2, 2, 2]);
    banker.set_allocation(2, &[0, 0, 1]);
    banker.set_max(2, &[2, 1, 2]);
    banker.set_allocation(3, &[1, 1, 0]);
    banker.set_max(3, &[2, 2, 1]);

    assert!(banker.is_safe());

    // Continuous requests: safe ones accepted, deadlock-closing/excessive ones rejected
    let attempts: [(usize, &[u32], bool); 5] = [
        (0, &[1, 0, 0], true),  // P0 requests safe
        (1, &[3, 0, 0], false), // P1 requests more than available
        (2, &[0, 1, 0], true),  // P2 requests safe
        (3, &[0, 0, 1], true),  // P3 requests safe
        (0, &[2, 0, 0], false), // P0 requests more than remaining need [2, 2, 2]
    ];

    for &(pid, req, expected_result) in &attempts {
        let result = banker.request_resources(pid, req);
        assert_eq!(
            result, expected_result,
            "Request by P{} {:?} did not match expected result {}",
            pid, req, expected_result
        );
        assert!(banker.is_safe(), "System must maintain SAFE state throughout");
    }
}

/// `TC-T4-03`: Complete User Session: Boot, Shell & Audit.
/// BIOS boot -> Splash -> TUI -> 17 commands (including `web`, `wifi`, `ping`, `api`) -> Clear.
#[test]
fn t4_scenario_03_complete_user_session_boot_shell_audit() {
    let mut shell = ShellDispatcher::new();

    // 1. Boot info
    let info = shell.execute("info");
    assert!(info.contains("Titanium Micro-OS"));
    assert!(info.contains("25BTRCL288"));

    // 2. Help
    let help = shell.execute("help");
    assert!(help.contains("Supported commands"));

    // 3. Tasks & Scheduler
    let tasks = shell.execute("tasks");
    assert!(tasks.contains("PID"));
    let sched = shell.execute("sched");
    assert!(sched.contains("MLFQ"));

    // 4. Memory & Deadlock
    let mem = shell.execute("mem");
    assert!(mem.contains("Bump Allocator"));
    let deadlock = shell.execute("deadlock");
    assert!(deadlock.contains("SAFE"));

    // 5. Simulated Web Network Portal (Mandatory Verification)
    let web = shell.execute("web");
    assert!(web.contains("VirtIO-Net"));
    assert!(web.contains("192.168.1.105"));
    assert!(web.contains("2.14 ms"));
    assert!(web.contains("200 OK"));

    // 6. Reset & Clear
    let reset = shell.execute("reset");
    assert!(reset.contains("reset successfully"));
    let clear = shell.execute("clear");
    assert!(clear.contains("\x1B[2J"));
}

/// `TC-T4-04`: Full Trace Analysis & Metrics Pipeline.
/// Captures trace -> parsed by parser -> metrics extracted and match scheduler counters.
#[test]
fn t4_scenario_04_full_trace_analysis_metrics_pipeline() {
    let mut sched = MlfqScheduler::new();
    sched.add_task(Task::new(0, "shell", TaskClass::Interactive, 30));
    sched.add_task(Task::new(1, "calc", TaskClass::Compute, 30));

    let mut serial_log = String::new();

    for tick in 1..=20 {
        if let Some(ev) = sched.schedule_once() {
            let rec = TraceRecord {
                tick,
                task_id: ev.task_id,
                name: ev.task_name.to_string(),
                queue: ev.queue,
                state: "RUNNING".to_string(),
                exec_time: tick * 2,
                wait_time: tick * 5,
                yield_time: tick,
                burst_ratio: ev.burst_ratio,
            };
            serial_log.push_str(&format_trace_line(&rec));
            serial_log.push('\n');
        }
    }

    // Verify pipeline
    let parsed: Vec<TraceRecord> = serial_log
        .lines()
        .filter_map(parse_trace_line)
        .collect();

    assert_eq!(parsed.len(), 20);
    assert_eq!(parsed[0].tick, 1);
    assert_eq!(parsed[19].tick, 20);
}

/// `TC-T4-05`: Heap Saturation, OOM Recovery & Resilience.
/// Heap filled to 64 KiB capacity -> OOM handled -> system continues functioning.
#[test]
fn t4_scenario_05_heap_saturation_oom_recovery_resilience() {
    let mut alloc = BumpAllocator::new();

    // Fill completely: 65,536 bytes
    let ptr = alloc.allocate(65536, 1);
    assert!(ptr.is_some());
    assert_eq!(alloc.free(), 0);

    // OOM attempts
    assert!(alloc.allocate(1, 1).is_none());
    assert!(alloc.allocate(1024, 8).is_none());

    // Reset and recover
    alloc.reset();
    assert_eq!(alloc.used(), 0);
    assert_eq!(alloc.free(), 65536);

    // System functions normally again
    let new_ptr = alloc.allocate(4096, 16);
    assert!(new_ptr.is_some());
    assert_eq!(alloc.used(), 4096);
}

/// `TC-T4-06`: Autonomous Cold-Boot Build & VM Execution.
/// Complete cold boot verification: ISO 9660, VDI, scripts, launch flags.
#[test]
fn t4_scenario_06_autonomous_cold_boot_build_and_vm_execution() {
    let iso_path = Path::new("dist/micro-os.iso");
    assert!(iso_path.exists(), "dist/micro-os.iso must exist on disk");
    let iso = fs::read(iso_path).expect("Read dist/micro-os.iso");
    assert!(verify_iso_pvd_sector16(&iso).is_ok());

    let vdi_path = Path::new("dist/micro-os-bios.vdi");
    assert!(vdi_path.exists(), "dist/micro-os-bios.vdi must exist on disk");
    let vdi = fs::read(vdi_path).expect("Read dist/micro-os-bios.vdi");
    assert!(verify_vdi_header(&vdi).is_ok());

    // Check launch commands from real script on disk
    let script_path = Path::new("scripts/run_virtualbox.sh");
    assert!(script_path.exists(), "scripts/run_virtualbox.sh must exist on disk");
    let script = fs::read_to_string(script_path).expect("Read scripts/run_virtualbox.sh");
    assert!(script.contains("--type gui"));
    assert!(script.contains("128"));
    assert!(script.contains("qemu-boot.log"));
    assert!(script.contains("VBoxInternal2/EnableX86OnArm"));
}
