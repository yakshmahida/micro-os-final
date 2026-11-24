//! Animated Boot Splash Screen for Titanium Micro-OS.
//!
//! CS2001 OS Capstone | Student: Yakshendrasinh R Mahida (USN: 25BTRCL288)
//! Renders a double-bordered CP437 splash box with project branding and a
//! PIT-timed 30-character animated progress bar (0% to 100% over 100 ticks @ 100 Hz).

use crate::pit;
use crate::serial_println;
use crate::vga::{self, Color};

pub const PROGRESS_BAR_WIDTH: usize = 30;
pub const SPLASH_START_ROW: usize = 9;
pub const SPLASH_BOX_HEIGHT: usize = 6;
pub const SPLASH_BOX_WIDTH: usize = 80;

/// Renders a single frame of the boot splash screen at the given progress percentage (0..=100).
pub fn render_frame(pct: usize) {
    let border_color = vga::color_attribute(Color::Cyan, Color::Black);
    let title_color = vga::color_attribute(Color::Yellow, Color::Black);
    let meta_color = vga::color_attribute(Color::White, Color::Black);
    let text_color = vga::color_attribute(Color::LightCyan, Color::Black);
    let bar_color = vga::color_attribute(Color::LightGreen, Color::Black);
    let empty_bar_color = vga::color_attribute(Color::DarkGray, Color::Black);
    let pct_color = vga::color_attribute(Color::Yellow, Color::Black);

    // Row 9: Top border ╔══...══╗
    vga::write_char_at(0, SPLASH_START_ROW, vga::BOX_DOUBLE_TOP_LEFT, border_color);
    for col in 1..79 {
        vga::write_char_at(col, SPLASH_START_ROW, vga::BOX_DOUBLE_HORIZONTAL, border_color);
    }
    vga::write_char_at(79, SPLASH_START_ROW, vga::BOX_DOUBLE_TOP_RIGHT, border_color);

    // Row 10: Project Title
    vga::write_char_at(0, SPLASH_START_ROW + 1, vga::BOX_DOUBLE_VERTICAL, border_color);
    vga::write_at(
        1,
        SPLASH_START_ROW + 1,
        "                        TITANIUM MICRO-OS v0.1.0                              ",
        title_color,
    );
    vga::write_char_at(79, SPLASH_START_ROW + 1, vga::BOX_DOUBLE_VERTICAL, border_color);

    // Row 11: Student & Capstone Metadata
    vga::write_char_at(0, SPLASH_START_ROW + 2, vga::BOX_DOUBLE_VERTICAL, border_color);
    vga::write_at(
        1,
        SPLASH_START_ROW + 2,
        "           CS2001 OS Capstone | Yakshendrasinh R Mahida (25BTRCL288)          ",
        meta_color,
    );
    vga::write_char_at(79, SPLASH_START_ROW + 2, vga::BOX_DOUBLE_VERTICAL, border_color);

    // Row 12: Blank separator line
    vga::write_char_at(0, SPLASH_START_ROW + 3, vga::BOX_DOUBLE_VERTICAL, border_color);
    for col in 1..79 {
        vga::write_char_at(col, SPLASH_START_ROW + 3, b' ', border_color);
    }
    vga::write_char_at(79, SPLASH_START_ROW + 3, vga::BOX_DOUBLE_VERTICAL, border_color);

    // Row 13: Progress Bar line:
    // "                   Booting Kernel... [" (col 1..22)
    // 30 chars of progress bar (col 23..52)
    // "] " (col 53..54)
    // "{pct:3}%" (col 55..58)
    // "                  " (col 59..78)
    vga::write_char_at(0, SPLASH_START_ROW + 4, vga::BOX_DOUBLE_VERTICAL, border_color);
    vga::write_at(
        1,
        SPLASH_START_ROW + 4,
        "                   Booting Kernel... [",
        text_color,
    );

    let filled = (pct * PROGRESS_BAR_WIDTH) / 100;
    let unfilled = PROGRESS_BAR_WIDTH - filled;

    for i in 0..filled {
        vga::write_char_at(23 + i, SPLASH_START_ROW + 4, vga::BLOCK_FULL, bar_color);
    }
    for i in 0..unfilled {
        vga::write_char_at(23 + filled + i, SPLASH_START_ROW + 4, b'-', empty_bar_color);
    }

    vga::write_at(53, SPLASH_START_ROW + 4, "] ", text_color);

    // 3-char right-aligned percentage
    if pct >= 100 {
        vga::write_at(55, SPLASH_START_ROW + 4, "100%", pct_color);
    } else if pct >= 10 {
        let tens = b'0' + ((pct / 10) % 10) as u8;
        let ones = b'0' + (pct % 10) as u8;
        vga::write_char_at(55, SPLASH_START_ROW + 4, b' ', pct_color);
        vga::write_char_at(56, SPLASH_START_ROW + 4, tens, pct_color);
        vga::write_char_at(57, SPLASH_START_ROW + 4, ones, pct_color);
        vga::write_char_at(58, SPLASH_START_ROW + 4, b'%', pct_color);
    } else {
        let ones = b'0' + (pct % 10) as u8;
        vga::write_at(55, SPLASH_START_ROW + 4, "  ", pct_color);
        vga::write_char_at(57, SPLASH_START_ROW + 4, ones, pct_color);
        vga::write_char_at(58, SPLASH_START_ROW + 4, b'%', pct_color);
    }

    for col in 59..79 {
        vga::write_char_at(col, SPLASH_START_ROW + 4, b' ', border_color);
    }
    vga::write_char_at(79, SPLASH_START_ROW + 4, vga::BOX_DOUBLE_VERTICAL, border_color);

    // Row 14: Bottom border ╚══...══╝
    vga::write_char_at(0, SPLASH_START_ROW + 5, vga::BOX_DOUBLE_BOTTOM_LEFT, border_color);
    for col in 1..79 {
        vga::write_char_at(col, SPLASH_START_ROW + 5, vga::BOX_DOUBLE_HORIZONTAL, border_color);
    }
    vga::write_char_at(79, SPLASH_START_ROW + 5, vga::BOX_DOUBLE_BOTTOM_RIGHT, border_color);
}

/// Runs the animated boot splash sequence driven by the PIT 100 Hz timer.
/// Advances 100 ticks (1000ms), clears the screen, and transitions to the TUI.
pub fn run() {
    serial_println!("[SPLASH] Animated boot splash screen starting...");
    serial_println!("[SPLASH] TITANIUM MICRO-OS v0.1.0 | USN: 25BTRCL288");

    vga::clear();

    let start_tick = pit::ticks();

    for pct in 0..=100 {
        render_frame(pct);

        let target = start_tick + pct as u64;
        let mut loop_count = 0usize;
        while pit::ticks() < target && loop_count < 100_000 {
            x86_64::instructions::hlt();
            loop_count += 1;
        }
    }

    serial_println!("[SPLASH] Progress: 100% [■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■]");
    serial_println!("[SPLASH] Boot splash animation complete. Clearing VGA display.");

    // Brief visual hold at 100% (5 ticks = 50ms)
    let hold_target = pit::ticks() + 5;
    let mut loop_count = 0usize;
    while pit::ticks() < hold_target && loop_count < 100_000 {
        x86_64::instructions::hlt();
        loop_count += 1;
    }

    // Clean transition to TUI Dashboard
    vga::clear();
}
