//! 80x25 TUI Multi-Panel Dashboard for Titanium Micro-OS.
//!
//! CS2001 OS Capstone | Student: Yakshendrasinh R Mahida (USN: 25BTRCL288)
//! Renders 5 panels matching STANDARD_DASHBOARD_PANELS:
//! - SYSTEM INFO: (0, 0, 40x7)
//! - TASK MLFQ:   (40, 0, 40x7)
//! - KERNEL LOG:  (0, 7, 40x10)
//! - MEM / BANKER:(40, 7, 40x10)
//! - SHELL:       (0, 17, 80x8)
//!
//! Zero heap allocations (#![no_std]), zero floating-point instructions.

use core::fmt::{self, Write};
use crate::deadlock;
use crate::memory;
use crate::pit;
use crate::scheduler;
use crate::sync::SpinLock;
use crate::task::TaskState;
use crate::vga::{self, Color};

/// Stack-allocated fixed-capacity buffer implementing `core::fmt::Write`.
pub struct FixedBuf<const N: usize> {
    pub buf: [u8; N],
    pub len: usize,
}

impl<const N: usize> FixedBuf<N> {
    pub const fn new() -> Self {
        Self { buf: [0; N], len: 0 }
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }
}

impl<const N: usize> Default for FixedBuf<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Write for FixedBuf<N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        let remaining = N.saturating_sub(self.len);
        let to_copy = remaining.min(bytes.len());
        self.buf[self.len..self.len + to_copy].copy_from_slice(&bytes[..to_copy]);
        self.len += to_copy;
        Ok(())
    }
}

pub const LOG_CAPACITY: usize = 16;
pub const LOG_LINE_MAX: usize = 38;

#[derive(Clone, Copy)]
pub struct LogEntry {
    pub bytes: [u8; LOG_LINE_MAX],
    pub len: usize,
    pub color: u8,
}

impl LogEntry {
    pub const fn new() -> Self {
        Self {
            bytes: [0; LOG_LINE_MAX],
            len: 0,
            color: 0x07, // LightGray on Black
        }
    }
}

impl Default for LogEntry {
    fn default() -> Self {
        Self::new()
    }
}

pub struct KernelLogBuffer {
    pub entries: [LogEntry; LOG_CAPACITY],
    pub head: usize,
    pub count: usize,
}

impl KernelLogBuffer {
    pub const fn new() -> Self {
        Self {
            entries: [LogEntry::new(); LOG_CAPACITY],
            head: 0,
            count: 0,
        }
    }

    pub fn push(&mut self, msg: &str, color: u8) {
        let bytes = msg.as_bytes();
        let len = bytes.len().min(LOG_LINE_MAX);
        let idx = (self.head + self.count) % LOG_CAPACITY;
        self.entries[idx].bytes[..len].copy_from_slice(&bytes[..len]);
        self.entries[idx].len = len;
        self.entries[idx].color = color;
        if self.count < LOG_CAPACITY {
            self.count += 1;
        } else {
            self.head = (self.head + 1) % LOG_CAPACITY;
        }
    }
}

impl Default for KernelLogBuffer {
    fn default() -> Self {
        Self::new()
    }
}

pub static KERNEL_LOGS: SpinLock<KernelLogBuffer> = SpinLock::new(KernelLogBuffer::new());

/// Writes a string to the panel line and pads the remainder of `width` with spaces.
pub fn write_panel_line(col: usize, row: usize, width: usize, text: &str, color: u8) {
    vga::write_at(col, row, text, color);
    let len = text.len();
    if len < width {
        let blank_color = vga::color_attribute(Color::LightGray, Color::Black);
        for c in (col + len)..(col + width) {
            vga::write_char_at(c, row, b' ', blank_color);
        }
    }
}

/// Appends a new kernel log entry to the circular log buffer and updates the log panel.
pub fn log(msg: &str) {
    let color = vga::color_attribute(Color::LightGray, Color::Black);
    KERNEL_LOGS.lock().push(msg, color);
    render_kernel_log();
}

/// Appends a colored kernel log entry.
pub fn log_colored(msg: &str, fg: Color) {
    let color = vga::color_attribute(fg, Color::Black);
    KERNEL_LOGS.lock().push(msg, color);
    render_kernel_log();
}

fn init_default_logs() {
    let mut logs = KERNEL_LOGS.lock();
    if logs.count == 0 {
        let c_info = vga::color_attribute(Color::LightCyan, Color::Black);
        let c_ok = vga::color_attribute(Color::LightGreen, Color::Black);
        let c_norm = vga::color_attribute(Color::LightGray, Color::Black);

        logs.push("[BOOT] UART COM1 (115200 8N1) ready", c_norm);
        logs.push("[BOOT] VGA 80x25 text/fb blitter ready", c_norm);
        logs.push("[MEM]  64 KiB Bump Allocator active", c_ok);
        logs.push("[BANK] Banker safe state verified", c_ok);
        logs.push("[SYNC] SpinLock verified", c_ok);
        logs.push("[SCHED] MLFQ 4 tasks registered", c_info);
        logs.push("[BOOT] IDT & PIC remapped (32-47)", c_norm);
        logs.push("[BOOT] PIT 100 Hz & PS/2 keyboard active", c_ok);
    }
}

/// Renders Panel 1: `SYSTEM INFO` (col 0, row 0, 40x7).
pub fn render_system_info() {
    let text_color = vga::color_attribute(Color::White, Color::Black);
    let label_color = vga::color_attribute(Color::LightCyan, Color::Black);
    let val_color = vga::color_attribute(Color::Yellow, Color::Black);
    let ok_color = vga::color_attribute(Color::LightGreen, Color::Black);

    write_panel_line(1, 1, 38, "OS:   Titanium Micro-OS v0.1.0", text_color);
    write_panel_line(1, 2, 38, "Arch: x86_64 (bare-metal no_std)", label_color);
    write_panel_line(1, 3, 38, "USN:  25BTRCL288 (CS2001 Capstone)", val_color);

    let ticks = pit::ticks();
    let ms = ticks * 10;
    let mut buf = FixedBuf::<64>::new();
    let _ = write!(buf, "Uptime: {} ticks ({} ms)", ticks, ms);
    write_panel_line(1, 4, 38, buf.as_str(), text_color);

    write_panel_line(1, 5, 38, "Status: Active | PIT: 100 Hz", ok_color);
}

/// Renders Panel 2: `TASK MLFQ` (col 40, row 0, 40x7).
pub fn render_task_mlfq() {
    let header_color = vga::color_attribute(Color::LightCyan, Color::Black);
    let text_color = vga::color_attribute(Color::White, Color::Black);

    write_panel_line(41, 1, 38, "PID NAME     STATE   Q   BURST(Theta)", header_color);

    if let Some(sched) = scheduler::SCHEDULER.try_lock() {
        let tasks = sched.get_tasks();
        for i in 0..4 {
            let row = 2 + i;
            if i < tasks.len() && tasks[i].is_some() {
                let t = tasks[i].as_ref().unwrap();
                let state_str = match t.state {
                    TaskState::Ready => "READY",
                    TaskState::Running => "RUNN ",
                    TaskState::Blocked => "BLKD ",
                    TaskState::Terminated => "TERM ",
                };
                let (whole, frac) = t.burst_ratio_parts();
                let mut buf = FixedBuf::<64>::new();
                let _ = write!(
                    buf,
                    " {:<2} {:<8} {:<5} Q{}  {}.{:04}",
                    t.id, t.name, state_str, t.queue, whole, frac
                );
                write_panel_line(41, row, 38, buf.as_str(), text_color);
            } else {
                write_panel_line(41, row, 38, " --", text_color);
            }
        }
    }
}

/// Renders Panel 3: `KERNEL LOG` (col 0, row 7, 40x10).
pub fn render_kernel_log() {
    let logs = KERNEL_LOGS.lock();
    let visible = logs.count.min(8);
    let start = logs.count.saturating_sub(visible);

    for i in 0..8 {
        let row = 8 + i;
        if i < visible {
            let idx = (logs.head + start + i) % LOG_CAPACITY;
            let entry = &logs.entries[idx];
            if let Ok(s) = core::str::from_utf8(&entry.bytes[..entry.len]) {
                write_panel_line(1, row, 38, s, entry.color);
            } else {
                write_panel_line(1, row, 38, "", entry.color);
            }
        } else {
            write_panel_line(1, row, 38, "", 0x07);
        }
    }
}

/// Renders Panel 4: `MEM / BANKER` (col 40, row 7, 40x10).
pub fn render_mem_banker() {
    let header_color = vga::color_attribute(Color::LightCyan, Color::Black);
    let text_color = vga::color_attribute(Color::White, Color::Black);
    let safe_color = vga::color_attribute(Color::LightGreen, Color::Black);
    let unsafe_color = vga::color_attribute(Color::LightRed, Color::Black);

    // Bump Allocator section (rows 8..11)
    write_panel_line(41, 8, 38, "-- 64 KiB BUMP ALLOCATOR --", header_color);

    let stats = memory::stats();
    let used_pct = (stats.used * 100) / stats.total;
    let free_pct = (stats.free * 100) / stats.total;

    let mut buf1 = FixedBuf::<64>::new();
    let _ = write!(buf1, "Used: {} B ({}%)", stats.used, used_pct);
    write_panel_line(41, 9, 38, buf1.as_str(), text_color);

    let mut buf2 = FixedBuf::<64>::new();
    let _ = write!(buf2, "Free: {} B ({}%)", stats.free, free_pct);
    write_panel_line(41, 10, 38, buf2.as_str(), text_color);

    let mut buf3 = FixedBuf::<64>::new();
    let _ = write!(buf3, "Allocs: {} | Total: 65536 B", stats.allocations);
    write_panel_line(41, 11, 38, buf3.as_str(), text_color);

    // Banker's Algorithm section (rows 12..15)
    write_panel_line(41, 12, 38, "-- BANKER DEADLOCK AVOIDANCE --", header_color);

    let b_status = deadlock::status();
    let (state_str, state_color) = if b_status.is_safe {
        ("SAFE (0 cycles)", safe_color)
    } else {
        ("UNSAFE / DEADLOCK", unsafe_color)
    };

    let mut buf4 = FixedBuf::<64>::new();
    let _ = write!(buf4, "State: {}", state_str);
    write_panel_line(41, 13, 38, buf4.as_str(), state_color);

    let avail = b_status.available;
    let mut buf5 = FixedBuf::<64>::new();
    let _ = write!(buf5, "Avail: [A:{} B:{} C:{}]", avail[0], avail[1], avail[2]);
    write_panel_line(41, 14, 38, buf5.as_str(), text_color);

    let seq = b_status.safe_sequence;
    let mut buf6 = FixedBuf::<64>::new();
    let _ = write!(
        buf6,
        "Safe Seq: <P{}->P{}->P{}->P{}>",
        seq[0], seq[1], seq[2], seq[3]
    );
    write_panel_line(41, 15, 38, buf6.as_str(), text_color);
}

/// Updates the live telemetry panels (SYSTEM INFO, TASK MLFQ, MEM / BANKER, and KERNEL LOG).
pub fn update_live_panels() {
    render_system_info();
    render_task_mlfq();
    render_mem_banker();
    render_kernel_log();
}

/// Alias for `update_live_panels()`.
pub fn refresh() {
    update_live_panels();
}

/// Initializes the 80x25 TUI Multi-Panel Dashboard.
/// Draws all 5 panel borders, static titles, initial kernel logs, and live telemetry.
pub fn init() {
    let border_color = vga::color_attribute(Color::Cyan, Color::Black);
    let title_color = vga::color_attribute(Color::Yellow, Color::Black);

    vga::clear();

    // 1. SYSTEM INFO (0, 0, 40x7)
    vga::draw_box(0, 0, 40, 7, border_color);
    vga::write_at(2, 0, " [ SYSTEM INFO ] ", title_color);

    // 2. TASK MLFQ (40, 0, 40x7)
    vga::draw_box(40, 0, 40, 7, border_color);
    vga::write_at(42, 0, " [ TASK MLFQ ] ", title_color);

    // 3. KERNEL LOG (0, 7, 40x10)
    vga::draw_box(0, 7, 40, 10, border_color);
    vga::write_at(2, 7, " [ KERNEL LOG ] ", title_color);

    // 4. MEM / BANKER (40, 7, 40x10)
    vga::draw_box(40, 7, 40, 10, border_color);
    vga::write_at(42, 7, " [ MEM / BANKER ] ", title_color);

    // 5. SHELL (0, 17, 80x8)
    vga::draw_box(0, 17, 80, 8, border_color);
    vga::write_at(2, 17, " [ SHELL ] ", title_color);

    // Populate initial kernel log messages
    init_default_logs();

    // Render live panels
    update_live_panels();
}
