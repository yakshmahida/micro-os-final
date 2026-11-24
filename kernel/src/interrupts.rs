//! 64-Bit Interrupt Descriptor Table (IDT) & Exception / Hardware IRQ Handlers
//!
//! Configures CPU exception handlers (Double Fault, Page Fault with CR2,
//! General Protection Fault, Breakpoint) and hardware IRQs (IRQ0 PIT Timer,
//! IRQ1 PS/2 Keyboard).

use crate::{keyboard, pic, pit, serial_println};
use spin::Once;
use x86_64::instructions::port::Port;
use x86_64::registers::control::Cr2;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

static IDT: Once<InterruptDescriptorTable> = Once::new();

/// Initializes the 64-bit IDT with CPU exceptions and hardware IRQ gates, then loads it.
pub fn init_idt() {
    let idt = IDT.call_once(|| {
        let mut idt = InterruptDescriptorTable::new();

        // CPU Exceptions
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.double_fault.set_handler_fn(double_fault_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt.general_protection_fault
            .set_handler_fn(general_protection_fault_handler);

        // Hardware IRQs
        idt[pic::PIC1_OFFSET].set_handler_fn(timer_interrupt_handler);
        idt[pic::PIC1_OFFSET + 1].set_handler_fn(keyboard_interrupt_handler);

        idt
    });

    idt.load();
}

/// Breakpoint exception handler (Vector 3).
extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    serial_println!("[EXCEPTION] Breakpoint at {:?}", stack_frame.instruction_pointer);
}

/// Double fault exception handler (Vector 8).
extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    serial_println!(
        "[FATAL EXCEPTION] Double Fault (code: {}):\n{:#?}",
        error_code,
        stack_frame
    );
    panic!("Kernel Double Fault Exception");
}

/// Page fault exception handler (Vector 14).
extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    let fault_addr = Cr2::read();
    serial_println!(
        "[EXCEPTION] Page Fault accessing address {:?} with flags: {:?}\n{:#?}",
        fault_addr,
        error_code,
        stack_frame
    );
    panic!("Kernel Page Fault Exception");
}

/// General protection fault exception handler (Vector 13).
extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    serial_println!(
        "[EXCEPTION] General Protection Fault (code: {}):\n{:#?}",
        error_code,
        stack_frame
    );
    panic!("Kernel General Protection Fault");
}

/// Hardware IRQ0 (Vector 32) timer interrupt handler from PIT 8253.
extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    pit::tick();
    crate::scheduler::tick();
    pic::end_of_interrupt(0);
}

/// Hardware IRQ1 (Vector 33) keyboard interrupt handler from PS/2 port 0x60.
extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let mut port: Port<u8> = Port::new(0x60);
    let scancode = unsafe { port.read() };
    keyboard::handle_scancode(scancode);
    pic::end_of_interrupt(1);
}
