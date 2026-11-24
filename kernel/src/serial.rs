//! UART 16550 Serial Driver for COM1 (0x3F8)
//!
//! Provides synchronized serial console output for boot diagnostics, trace
//! capture, and kernel debugging over COM1 at 115200 baud, 8N1.

use core::fmt::{self, Write};
use spin::Mutex;
use uart_16550::SerialPort;

/// Global synchronized UART1 serial console instance on COM1 (0x3F8).
pub static SERIAL1: Mutex<SerialPort> = Mutex::new(unsafe { SerialPort::new(0x3F8) });

/// Initializes COM1 serial port to 115200 baud, 8N1.
pub fn init() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        SERIAL1.lock().init();
    });
}

/// Sends a single byte to the serial console.
pub fn send_byte(byte: u8) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        SERIAL1.lock().send(byte);
    });
}

/// Helper function invoked by formatting macros to print formatted text.
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        SERIAL1
            .lock()
            .write_fmt(args)
            .expect("Printing to serial failed");
    });
}

/// Prints formatted text to the serial console without a trailing newline.
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*));
    };
}

/// Prints formatted text to the serial console with a trailing newline.
#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($($arg:tt)*) => ($crate::serial_print!("{}\n", format_args!($($arg)*)));
}
