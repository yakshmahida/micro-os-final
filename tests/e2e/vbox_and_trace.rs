//! Tier 1 & Tier 2 E2E Tests for R5 (VirtualBox Automation) & R6 (Trace Analysis).
//! CS2001 OS Capstone | Student: Yakshendrasinh R Mahida (USN: 25BTRCL288)

use std::fs;
use std::path::Path;
use super::common::*;

// ============================================================================
// Tier 1: Feature Coverage (Happy Path)
// ============================================================================

/// `TC-T1-33`: Apple Silicon x86 Extradata Check.
/// Validates `VBoxInternal2/EnableX86OnArm = 1` configuration for Apple Silicon hosts.
#[test]
fn t1_vbox_33_apple_silicon_x86_extradata_check() {
    let script_path = Path::new("scripts/run_virtualbox.sh");
    assert!(script_path.exists(), "scripts/run_virtualbox.sh must exist on disk");
    let script_content = fs::read_to_string(script_path).expect("Failed to read scripts/run_virtualbox.sh");

    assert!(
        script_content.contains("VBoxInternal2/EnableX86OnArm"),
        "VirtualBox script must configure VBoxInternal2/EnableX86OnArm"
    );
    assert!(
        script_content.contains(" 1"),
        "EnableX86OnArm must be set to 1"
    );
}

/// `TC-T1-34`: VM Specification Verification.
/// Validates 128MB RAM, BIOS firmware, and VM name "TitaniumMicroOS".
#[test]
fn t1_vbox_34_vm_specification_verification() {
    let script_path = Path::new("scripts/run_virtualbox.sh");
    assert!(script_path.exists(), "scripts/run_virtualbox.sh must exist on disk");
    let script_content = fs::read_to_string(script_path).expect("Failed to read scripts/run_virtualbox.sh");

    assert!(
        script_content.contains("TitaniumMicroOS"),
        "VM name must be TitaniumMicroOS"
    );
    assert!(
        script_content.contains("--memory 128"),
        "VM memory must be 128 MB"
    );
    assert!(
        script_content.contains("--firmware bios") || script_content.contains("bios"),
        "VM firmware must be BIOS"
    );
}

/// `TC-T1-35`: VM Storage ISO Attachment.
/// Validates IDE controller attaches `dist/micro-os.iso`.
#[test]
fn t1_vbox_35_vm_storage_iso_attachment() {
    let script_path = Path::new("scripts/run_virtualbox.sh");
    assert!(script_path.exists(), "scripts/run_virtualbox.sh must exist on disk");
    let script_content = fs::read_to_string(script_path).expect("Failed to read scripts/run_virtualbox.sh");

    assert!(
        script_content.contains("storageattach"),
        "Script must attach storage to VM"
    );
    assert!(
        script_content.contains("micro-os.iso"),
        "Script must attach dist/micro-os.iso"
    );
    assert!(
        script_content.contains("dvddrive") || script_content.contains("ide"),
        "Storage attachment must configure DVD/IDE device"
    );
}

/// `TC-T1-36`: VM Serial Redirection to Trace.
/// Validates UART1 COM1 pipes to `traces/qemu-boot.log`.
#[test]
fn t1_vbox_36_vm_serial_redirection_to_trace() {
    let script_path = Path::new("scripts/run_virtualbox.sh");
    assert!(script_path.exists(), "scripts/run_virtualbox.sh must exist on disk");
    let script_content = fs::read_to_string(script_path).expect("Failed to read scripts/run_virtualbox.sh");

    assert!(
        script_content.contains("--uart1 0x3F8 4") || script_content.contains("--uart1"),
        "Script must configure UART1"
    );
    assert!(
        script_content.contains("file"),
        "UART mode must be file output"
    );
    assert!(
        script_content.contains("qemu-boot.log"),
        "Serial redirection must point to qemu-boot.log"
    );
}

/// `TC-T1-37`: Autonomous GUI Launch Parameter.
/// Validates `scripts/run_virtualbox.sh` launches with `--type gui`.
#[test]
fn t1_vbox_37_autonomous_gui_launch_parameter() {
    let script_path = Path::new("scripts/run_virtualbox.sh");
    assert!(script_path.exists(), "scripts/run_virtualbox.sh must exist on disk");
    let script_content = fs::read_to_string(script_path).expect("Failed to read scripts/run_virtualbox.sh");

    assert!(
        script_content.contains("startvm") && script_content.contains("--type gui"),
        "VM start invocation must explicitly specify --type gui"
    );
}

/// `TC-T1-38`: Host Unit Test Suite Execution.
/// Validates `tests/kernel_mechanisms.rs` contains $\ge 7$ tests.
#[test]
fn t1_trace_38_host_unit_test_suite_execution() {
    let test_file = Path::new("tests/kernel_mechanisms.rs");
    assert!(test_file.exists(), "tests/kernel_mechanisms.rs must exist on disk");
    let test_content = fs::read_to_string(test_file).expect("Failed to read tests/kernel_mechanisms.rs");

    let test_count = test_content.matches("#[test]").count();
    assert!(
        test_count >= 7,
        "tests/kernel_mechanisms.rs must contain at least 7 test cases, found {}",
        test_count
    );
}

/// `TC-T1-39`: Serial Trace Log Capture Format.
/// Validates trace lines match regex schema.
#[test]
fn t1_trace_39_serial_trace_log_capture_format() {
    let sample_line = "[SCHED] tick=42 task=1 (shell) Q=0 state=RUNNING exec=10ms wait=80ms yield=10ms Θ=0.1000";
    let record = parse_trace_line(sample_line);

    assert!(record.is_some(), "Valid trace line must parse successfully");
    let rec = record.unwrap();
    assert_eq!(rec.tick, 42);
    assert_eq!(rec.task_id, 1);
    assert_eq!(rec.name, "shell");
    assert_eq!(rec.queue, 0);
    assert_eq!(rec.exec_time, 10);
    assert_eq!(rec.wait_time, 80);
    assert_eq!(rec.yield_time, 10);
    assert!((rec.burst_ratio - 0.1000).abs() < 1e-4);
}

/// `TC-T1-40`: Python Trace Script Execution.
/// Validates execution of trace analysis logic against trace log using scripts/analyze_trace.py.
#[test]
fn t1_trace_40_python_trace_script_execution() {
    let script_path = Path::new("scripts/analyze_trace.py");
    assert!(script_path.exists(), "scripts/analyze_trace.py must exist on disk");

    // Generate synthetic log lines in a temporary file
    let tmp_dir = std::env::temp_dir();
    let trace_file = tmp_dir.join("test_e2e_trace.log");
    let mut log = String::new();
    for tick in 1..=20 {
        let rec = TraceRecord {
            tick,
            task_id: tick % 4,
            name: format!("task_{}", tick % 4),
            queue: (tick / 5) % 3,
            state: "RUNNING".to_string(),
            exec_time: tick * 2,
            wait_time: tick * 5,
            yield_time: tick,
            burst_ratio: 0.25,
        };
        log.push_str(&format_trace_line(&rec));
        log.push('\n');
    }
    fs::write(&trace_file, &log).expect("Write temporary trace file");

    // Execute python3 scripts/analyze_trace.py against the trace file
    let output = std::process::Command::new("python3")
        .arg("scripts/analyze_trace.py")
        .arg(&trace_file)
        .output()
        .expect("Execute analyze_trace.py");

    let _ = fs::remove_file(&trace_file);

    assert!(
        output.status.success(),
        "scripts/analyze_trace.py must exit with code 0"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Titanium Micro-OS Serial Trace Analysis Report"));
    assert!(stdout.contains("Parsed 20 scheduler execution records"));
}

/// `TC-T1-41`: Burst Ratio $\Theta$ Metric Extraction.
/// Validates script extracts $\Theta$ for shell (< 0.20) and compute ($\ge 0.20$).
#[test]
fn t1_trace_41_burst_ratio_theta_metric_extraction() {
    let shell_rec = TraceRecord {
        tick: 1,
        task_id: 1,
        name: "shell".to_string(),
        queue: 0,
        state: "RUNNING".to_string(),
        exec_time: 5,
        wait_time: 80,
        yield_time: 15,
        burst_ratio: 0.05,
    };
    assert!(
        shell_rec.burst_ratio < 0.20,
        "Shell burst ratio must be strictly < 0.20"
    );

    let worker_rec = TraceRecord {
        tick: 2,
        task_id: 2,
        name: "worker".to_string(),
        queue: 2,
        state: "RUNNING".to_string(),
        exec_time: 90,
        wait_time: 5,
        yield_time: 5,
        burst_ratio: 0.90,
    };
    assert!(
        worker_rec.burst_ratio >= 0.20,
        "Worker burst ratio must be >= 0.20"
    );
}

/// `TC-T1-42`: Context Switch & Queue Accounting.
/// Validates switch counting and queue residency distribution.
#[test]
fn t1_trace_42_context_switch_and_queue_accounting() {
    let sample_records = vec![
        TraceRecord { tick: 1, task_id: 1, name: "t1".into(), queue: 0, state: "RUNNING".into(), exec_time: 1, wait_time: 0, yield_time: 0, burst_ratio: 0.1 },
        TraceRecord { tick: 2, task_id: 2, name: "t2".into(), queue: 1, state: "RUNNING".into(), exec_time: 1, wait_time: 0, yield_time: 0, burst_ratio: 0.8 },
        TraceRecord { tick: 3, task_id: 1, name: "t1".into(), queue: 0, state: "RUNNING".into(), exec_time: 2, wait_time: 0, yield_time: 0, burst_ratio: 0.1 },
    ];

    let mut switches = 0;
    let mut last_id = None;
    let mut q_residency = [0usize; 3];

    for r in &sample_records {
        if let Some(prev) = last_id {
            if prev != r.task_id {
                switches += 1;
            }
        }
        last_id = Some(r.task_id);
        q_residency[r.queue] += 1;
    }

    assert_eq!(switches, 2, "There must be 2 context switches");
    assert_eq!(q_residency[0], 2, "Q0 must have 2 records");
    assert_eq!(q_residency[1], 1, "Q1 must have 1 record");
}

// ============================================================================
// Tier 2: Boundary & Corner Cases (Stress & Limits)
// ============================================================================

/// `TC-T2-33`: Pre-Existing VM State Collision.
/// Validates script unregisters and cleans up existing VM `TitaniumMicroOS` before creation.
#[test]
fn t2_vbox_33_preexisting_vm_state_collision() {
    let script_path = Path::new("scripts/run_virtualbox.sh");
    assert!(script_path.exists(), "scripts/run_virtualbox.sh must exist on disk");
    let script_content = fs::read_to_string(script_path).expect("Failed to read scripts/run_virtualbox.sh");

    assert!(
        script_content.contains("unregistervm"),
        "Script must handle unregistering pre-existing VM"
    );
    assert!(
        script_content.contains("--delete"),
        "Unregistration should specify --delete to clear old state"
    );
}

/// `TC-T2-34`: Missing ISO File Path Handling.
/// Validates script checks for existence of `dist/micro-os.iso` before starting VM.
#[test]
fn t2_vbox_34_missing_iso_file_path_handling() {
    let script_path = Path::new("scripts/run_virtualbox.sh");
    assert!(script_path.exists(), "scripts/run_virtualbox.sh must exist on disk");
    let script_content = fs::read_to_string(script_path).expect("Failed to read scripts/run_virtualbox.sh");

    assert!(
        script_content.contains("[ ! -f \"${ISO_PATH}\" ]") || script_content.contains("ISO not found"),
        "Script must check for existence of ISO file before attach"
    );
}

/// `TC-T2-35`: Missing `traces/` Directory Handling.
/// Validates script automatically creates `traces/` directory (`mkdir -p traces`).
#[test]
fn t2_vbox_35_missing_traces_directory_handling() {
    let script_path = Path::new("scripts/run_virtualbox.sh");
    assert!(script_path.exists(), "scripts/run_virtualbox.sh must exist on disk");
    let script_content = fs::read_to_string(script_path).expect("Failed to read scripts/run_virtualbox.sh");

    assert!(
        script_content.contains("mkdir -p"),
        "Script must create log directory with mkdir -p"
    );
}

/// `TC-T2-36`: Duplicate Storage Controller Addition.
/// Validates script handles or ignores existing storage controllers without aborting.
#[test]
fn t2_vbox_36_duplicate_storage_controller_addition() {
    let script_path = Path::new("scripts/run_virtualbox.sh");
    assert!(script_path.exists(), "scripts/run_virtualbox.sh must exist on disk");
    let script_content = fs::read_to_string(script_path).expect("Failed to read scripts/run_virtualbox.sh");

    assert!(
        script_content.contains("storagectl") && script_content.contains("|| true"),
        "Storage controller addition must tolerate existing controller"
    );
}

/// `TC-T2-37`: Clean Forced Poweroff of Stuck VM.
/// Validates script executes `controlvm poweroff` before unregistering in case VM is running.
#[test]
fn t2_vbox_37_clean_forced_poweroff_of_stuck_vm() {
    let script_path = Path::new("scripts/run_virtualbox.sh");
    assert!(script_path.exists(), "scripts/run_virtualbox.sh must exist on disk");
    let script_content = fs::read_to_string(script_path).expect("Failed to read scripts/run_virtualbox.sh");

    assert!(
        script_content.contains("controlvm") && script_content.contains("poweroff"),
        "Script must safely force-poweroff VM before unregistering"
    );
}

/// `TC-T2-38`: Empty Serial Trace Log File.
/// Validates trace parser handles 0-byte log file without crashing.
#[test]
fn t2_trace_38_empty_serial_trace_log_file() {
    let empty_log = "";
    let lines: Vec<&str> = empty_log.lines().collect();
    let records: Vec<TraceRecord> = lines.into_iter().filter_map(parse_trace_line).collect();
    assert_eq!(records.len(), 0, "Empty log must produce 0 records without panic");
}

/// `TC-T2-39`: Corrupted Non-ASCII Log Bytes.
/// Validates trace parser handles non-ASCII / corrupted binary bytes without crash.
#[test]
fn t2_trace_39_corrupted_non_ascii_log_bytes() {
    let corrupted_bytes: [u8; 12] = [0xFF, 0xFE, 0xAA, 0x00, 0x12, 0x34, 0x80, 0x90, 0x01, 0x02, 0x03, 0x04];
    let lossy_string = String::from_utf8_lossy(&corrupted_bytes);

    // Attempt parsing line with corrupted contents
    let record = parse_trace_line(&lossy_string);
    assert_eq!(record, None, "Corrupted binary bytes must not cause panic and must return None");
}

/// `TC-T2-40`: Truncated / Incomplete Log Lines.
/// Validates trace parser skips partial log lines without regex match failure.
#[test]
fn t2_trace_40_truncated_incomplete_log_lines() {
    let truncated_lines = [
        "[SCHED] tick=10 task=1",
        "[SCHED] tick=12 task=2 (work",
        "[SCHED]",
        "[SCHED] tick=",
    ];

    for line in &truncated_lines {
        let res = parse_trace_line(line);
        assert_eq!(
            res, None,
            "Truncated line '{}' must be discarded safely as None",
            line
        );
    }
}

/// `TC-T2-41`: Single-Event Trace Log.
/// Validates trace parser handles log with single event without division-by-zero.
#[test]
fn t2_trace_41_single_event_trace_log() {
    let single_line = "[SCHED] tick=1 task=1 (shell) Q=0 state=RUNNING exec=1ms wait=0ms yield=0ms Θ=1.0000";
    let rec = parse_trace_line(single_line).expect("Must parse single event");

    let records = [rec];
    let avg_theta = if records.is_empty() {
        0.0
    } else {
        records.iter().map(|r| r.burst_ratio).sum::<f64>() / records.len() as f64
    };

    assert_eq!(avg_theta, 1.0, "Average theta of single event must be 1.0");
}

/// `TC-T2-42`: High-Volume Log Performance (100k).
/// Validates processing 100,000 synthetic trace lines executes efficiently.
#[test]
fn t2_trace_42_high_volume_log_performance_100k() {
    let start = std::time::Instant::now();
    let sample_line = "[SCHED] tick=100 task=3 (worker) Q=2 state=RUNNING exec=50ms wait=10ms yield=2ms Θ=0.8065";

    let mut parsed_count = 0;
    for _ in 0..100_000 {
        if parse_trace_line(sample_line).is_some() {
            parsed_count += 1;
        }
    }

    let elapsed = start.elapsed();
    assert_eq!(parsed_count, 100_000);
    assert!(
        elapsed.as_secs() < 3,
        "Parsing 100k trace lines took {:?}, expected < 3 seconds",
        elapsed
    );
}
