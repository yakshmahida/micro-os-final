//! PIT 8253/8254 Programmable Interval Timer Driver
//!
//! Configures Channel 0 of the PIT for periodic timer interrupts on IRQ0
//! at a target frequency (default 100 Hz, 10ms period). Tracks elapsed
//! system ticks since boot for scheduling, delays, and animation timing.

use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::instructions::port::Port;

pub const PIT_BASE_FREQUENCY: usize = 1_193_182;
pub const PIT_CHANNEL_0: u16 = 0x40;
pub const PIT_COMMAND: u16 = 0x43;

// Channel 0, Access mode: lobyte/hibyte, Mode 3 (Square Wave), 16-bit binary
const PIT_MODE_3: u8 = 0b0011_0110; // 0x36

/// Monotonic tick counter incremented every IRQ0 timer interrupt.
static TICKS: AtomicU64 = AtomicU64::new(0);

/// Initializes PIT Channel 0 at the requested frequency in Hz.
/// For 100 Hz, divisor is 11932 (0x2E9C), firing an interrupt every 10ms.
pub fn init(freq_hz: usize) {
    let divisor = (PIT_BASE_FREQUENCY / freq_hz) as u16;

    let mut cmd_port: Port<u8> = Port::new(PIT_COMMAND);
    let mut data_port: Port<u8> = Port::new(PIT_CHANNEL_0);

    unsafe {
        cmd_port.write(PIT_MODE_3);
        // Write LSB followed by MSB
        data_port.write((divisor & 0xFF) as u8);
        data_port.write(((divisor >> 8) & 0xFF) as u8);
    }
}

/// Increments the global monotonic tick counter. Called from the IRQ0 interrupt handler.
pub fn tick() {
    TICKS.fetch_add(1, Ordering::Relaxed);
}

/// Returns the number of timer ticks elapsed since kernel initialization.
pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}
