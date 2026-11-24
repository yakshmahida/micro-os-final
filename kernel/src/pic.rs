//! 8259 PIC (Programmable Interrupt Controller) Driver
//!
//! Remaps the 8259 PIC master and slave cascade to interrupt vectors 32..47
//! (eliminating conflicts with x86 CPU exception vectors 0..31), unmasks
//! IRQ0 (PIT timer) and IRQ1 (PS/2 keyboard), and handles EOI acknowledgments.

use pic8259::ChainedPics;
use spin::Mutex;
use x86_64::instructions::port::Port;

pub const PIC1_OFFSET: u8 = 32;
pub const PIC2_OFFSET: u8 = 40;

pub const PIC1_COMMAND: u16 = 0x20;
pub const PIC1_DATA: u16 = 0x21;
pub const PIC2_COMMAND: u16 = 0xA0;
pub const PIC2_DATA: u16 = 0xA1;
pub const PIC_EOI: u8 = 0x20;

/// Global synchronized 8259 PIC cascade controller.
pub static PICS: Mutex<ChainedPics> =
    Mutex::new(unsafe { ChainedPics::new(PIC1_OFFSET, PIC2_OFFSET) });

/// Remaps Master and Slave 8259 PICs to interrupt vectors 32..47,
/// unmasking IRQ0 (timer) and IRQ1 (keyboard).
pub fn init() {
    unsafe {
        PICS.lock().initialize();

        // Configure interrupt masks directly:
        // Master: Unmask IRQ0 (bit 0 = timer) and IRQ1 (bit 1 = keyboard).
        // Also keep IRQ2 unmasked for slave cascade line -> 0b1111_1000 = 0xF8.
        let mut data1: Port<u8> = Port::new(PIC1_DATA);
        data1.write(0xF8);

        // Slave: Mask all lines -> 0xFF
        let mut data2: Port<u8> = Port::new(PIC2_DATA);
        data2.write(0xFF);
    }
}

/// Sends End of Interrupt (EOI) byte 0x20 for the given IRQ line (0..15).
pub fn end_of_interrupt(irq: u8) {
    unsafe {
        PICS.lock().notify_end_of_interrupt(PIC1_OFFSET + irq);
    }
}

/// Sends End of Interrupt for the given interrupt vector (32..47).
pub fn notify_end_of_interrupt(vector: u8) {
    unsafe {
        PICS.lock().notify_end_of_interrupt(vector);
    }
}
