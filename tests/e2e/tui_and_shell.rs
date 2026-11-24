//! Tier 1 & Tier 2 E2E Tests for R4: Modern TUI Dashboard, Shell & Web Command.
//! CS2001 OS Capstone | Student: Yakshendrasinh R Mahida (USN: 25BTRCL288)

use super::common::*;

// ============================================================================
// Tier 1: Feature Coverage (Happy Path)
// ============================================================================

/// `TC-T1-27`: Animated Boot Splash Execution.
/// Validates progress bar rendering 0% to 100% and metadata banner.
#[test]
fn t1_shell_27_animated_boot_splash_execution() {
    let frame_0 = render_splash_frame(0);
    assert!(frame_0.contains("0%"));
    assert!(frame_0.contains("25BTRCL288"));
    assert!(frame_0.to_uppercase().contains("TITANIUM MICRO-OS"));

    let frame_50 = render_splash_frame(50);
    assert!(frame_50.contains("50%"));

    let frame_100 = render_splash_frame(100);
    assert!(frame_100.contains("100%"));
    assert!(frame_100.contains("■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■"));
}

/// `TC-T1-28`: Multi-Panel TUI Grid Layout.
/// Validates 80x25 dashboard renders all 5 panels with borders.
#[test]
fn t1_shell_28_multi_panel_tui_grid_layout() {
    assert_eq!(
        STANDARD_DASHBOARD_PANELS.len(),
        5,
        "Dashboard must contain 5 distinct panels"
    );

    // Check all panels fit inside 80x25
    for p in &STANDARD_DASHBOARD_PANELS {
        assert!(
            p.col + p.width <= 80,
            "Panel '{}' width exceeds screen width",
            p.title
        );
        assert!(
            p.row + p.height <= 25,
            "Panel '{}' height exceeds screen height",
            p.title
        );
    }
}

/// `TC-T1-29`: Shell Command: `help`.
/// Validates `help` lists all 17 supported shell commands.
#[test]
fn t1_shell_29_shell_command_help() {
    let mut shell = ShellDispatcher::new();
    let output = shell.execute("help");

    let required_commands = [
        "help", "tasks", "sched", "mem", "deadlock", "info", "reset", "clear", "web",
        "wifi", "net", "ip", "ping", "dns", "ports", "api", "trace"
    ];

    for &cmd in &required_commands {
        assert!(
            output.contains(cmd),
            "Help output must document command '{}'",
            cmd
        );
    }
}

/// `TC-T1-30`: Shell Commands: `tasks` & `sched`.
/// Validates task list and MLFQ scheduling statistics display.
#[test]
fn t1_shell_30_shell_commands_tasks_and_sched() {
    let mut shell = ShellDispatcher::new();

    // Test 'tasks'
    let tasks_out = shell.execute("tasks");
    assert!(tasks_out.contains("PID"));
    assert!(tasks_out.contains("NAME"));
    assert!(tasks_out.contains("shell"));
    assert!(tasks_out.contains("worker"));
    assert!(tasks_out.contains("BURST_RATIO"));

    // Test 'sched'
    let sched_out = shell.execute("sched");
    assert!(sched_out.contains("MLFQ"));
    assert!(sched_out.contains("Q0"));
    assert!(sched_out.contains("Q1"));
    assert!(sched_out.contains("Q2"));
    assert!(sched_out.contains("50ms"));
    assert!(sched_out.contains("0.20"));
}

/// `TC-T1-31`: Shell Commands: `mem` & `deadlock`.
/// Validates heap metrics and Banker safe state output.
#[test]
fn t1_shell_31_shell_commands_mem_and_deadlock() {
    let mut shell = ShellDispatcher::new();

    // Test 'mem'
    let mem_out = shell.execute("mem");
    assert!(mem_out.contains("Bump Allocator"));
    assert!(mem_out.contains("65536"));
    assert!(mem_out.contains("64 KiB"));
    assert!(mem_out.contains("Allocated:"));
    assert!(mem_out.contains("Free:"));

    // Test 'deadlock'
    let deadlock_out = shell.execute("deadlock");
    assert!(deadlock_out.contains("Banker's Deadlock Avoidance"));
    assert!(deadlock_out.contains("SAFE"));
    assert!(deadlock_out.contains("Safe Sequence:"));
}

/// `TC-T1-32`: Shell Command: `web` Portal.
/// Specifically verify that the `web` command outputs simulated network metrics
/// (VirtIO-Net, IP 192.168.1.105, Ping 2.14 ms, HTTP 200 OK) and mock portal banner.
#[test]
fn t1_shell_32_shell_command_web_portal() {
    let mut shell = ShellDispatcher::new();
    let web_out = shell.execute("web");

    // Mandatory specific verifications per dispatch prompt
    assert!(
        web_out.contains("VirtIO-Net"),
        "Web command must output VirtIO-Net network adapter"
    );
    assert!(
        web_out.contains("192.168.1.105"),
        "Web command must output IP address 192.168.1.105"
    );
    assert!(
        web_out.contains("2.14 ms"),
        "Web command must output Ping 2.14 ms"
    );
    assert!(
        web_out.contains("200 OK"),
        "Web command must output HTTP 200 OK"
    );
    assert!(
        web_out.contains("TITANIUM") || web_out.contains("PORTAL"),
        "Web command must display mock network portal banner"
    );
}

// ============================================================================
// Tier 2: Boundary & Corner Cases (Stress & Limits)
// ============================================================================

/// `TC-T2-28`: Shell Command Buffer Overflow.
/// Validates keystroke sequence exceeding 128 characters is clamped safely.
#[test]
fn t2_shell_28_shell_command_buffer_overflow() {
    let mut shell = ShellDispatcher::new();
    // 200 character string of 'a's
    let long_cmd = "a".repeat(200);
    let output = shell.execute(&long_cmd);

    // Shell must not crash, must return an error for unknown command
    assert!(
        output.contains("Unknown command"),
        "Overlong command must return unknown command message"
    );
    // Recorded history item must be clamped to at most 128 chars
    let last_history = shell.history().last().unwrap();
    assert!(
        last_history.len() <= 128,
        "History command must be clamped to 128 characters, found {}",
        last_history.len()
    );
}

/// `TC-T2-29`: Shell Empty Command Input.
/// Validates user pressing Enter without typing characters produces clean no-op.
#[test]
fn t2_shell_29_shell_empty_command_input() {
    let mut shell = ShellDispatcher::new();
    let output = shell.execute("");
    assert_eq!(output, "", "Empty input must return empty string without error");
}

/// `TC-T2-30`: Shell Whitespace-Only Command Input.
/// Validates spaces and tabs are ignored as clean no-op.
#[test]
fn t2_shell_30_shell_whitespace_only_command_input() {
    let mut shell = ShellDispatcher::new();
    let output = shell.execute("   \t  \n  ");
    assert_eq!(
        output, "",
        "Whitespace-only input must return empty string without error"
    );
}

/// `TC-T2-31`: Shell Interstitial Space Normalization.
/// Validates spaces surrounding valid command (`  mem   `, `   web   `) are normalized.
#[test]
fn t2_shell_31_shell_interstitial_space_normalization() {
    let mut shell = ShellDispatcher::new();

    let mem_out = shell.execute("   mem   ");
    assert!(
        mem_out.contains("Bump Allocator"),
        "Surrounding spaces on 'mem' must be normalized"
    );

    let web_out = shell.execute("  web      ");
    assert!(
        web_out.contains("VirtIO-Net"),
        "Surrounding spaces on 'web' must be normalized"
    );

    let help_out = shell.execute("\t  help  \t");
    assert!(
        help_out.contains("Supported commands"),
        "Tabs and spaces on 'help' must be normalized"
    );
}

/// `TC-T2-32`: Shell Unrecognized Arbitrary String.
/// Validates unknown command outputs informative message without panic.
#[test]
fn t2_shell_32_shell_unrecognized_arbitrary_string() {
    let mut shell = ShellDispatcher::new();
    let output = shell.execute("foobar_command_123");

    assert!(
        output.contains("Unknown command: 'foobar_command_123'"),
        "Unknown command must produce informative error message"
    );
    assert!(
        output.contains("Type 'help'"),
        "Error message should guide user to type 'help'"
    );
}
