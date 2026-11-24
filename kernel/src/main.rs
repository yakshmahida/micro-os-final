//! Titanium Micro-OS Bare-Metal x86_64 Kernel
//!
//! Entry point for the CS2001 OS Capstone (Student: Yakshendrasinh R Mahida, USN: 25BTRCL288).
//! Boots into 64-bit long mode via `bootloader_api`, establishes bare-metal hardware
//! drivers (VGA text / framebuffer blitter, UART 16550, 64-bit IDT, PIC 8259 remap,
//! PIT 8253 @ 100 Hz, PS/2 keyboard ring buffer), initializes kernel core mechanisms
//! (64 KiB Bump Allocator, Dijkstra's Banker's Algorithm, SpinLock, 3-Queue MLFQ),
//! runs animated boot splash, displays 80x25 TUI multi-panel dashboard, and enters
//! the interactive shell event loop.

#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

pub mod deadlock;
pub mod font;
pub mod interrupts;
pub mod keyboard;
pub mod memory;
pub mod pic;
pub mod pit;
pub mod scheduler;
pub mod serial;
pub mod shell;
pub mod splash;
pub mod sync;
pub mod task;
pub mod ui;
pub mod vga;

use core::panic::PanicInfo;
use task::{Task, TaskClass};

/// Bootloader configuration requesting dynamic physical memory mapping.
pub static BOOTLOADER_CONFIG: bootloader_api::BootloaderConfig = {
    let mut config = bootloader_api::BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(bootloader_api::config::Mapping::Dynamic);
    config
};

bootloader_api::entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

/// Bare-metal entry point receiving `BootInfo` from the bootloader.
fn kernel_main(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    // 1. Initialize serial console on COM1 (115200 baud, 8N1)
    serial::init();
    serial_println!("============================================================");
    serial_println!(" Titanium Micro-OS: Bare-Metal x86_64 Kernel");
    serial_println!(" CS2001 OS Capstone | Student: Yakshendrasinh R Mahida (25BTRCL288)");
    serial_println!("============================================================");
    serial_println!("[BOOT] UART 16550 serial console initialized on COM1 (115200 baud, 8N1).");

    // 2. Query physical memory mapping and framebuffer
    let phys_mem_offset = boot_info.physical_memory_offset.into_option();
    serial_println!("[BOOT] Physical memory mapping offset: {:?}", phys_mem_offset);

    let framebuffer = boot_info.framebuffer.as_mut();
    if framebuffer.is_some() {
        serial_println!("[BOOT] Graphical framebuffer detected.");
    } else {
        serial_println!("[BOOT] VGA text-mode fallback active.");
    }

    // 3. Initialize VGA display engine and CP437 box drawing
    vga::init(phys_mem_offset, framebuffer);
    serial_println!("[BOOT] VGA 80x25 display engine initialized at 0xB8000.");

    // 4. Initialize 64 KiB Bump Allocator (pre-allocate 16 KiB baseline)
    memory::init();
    memory::allocate(16384, 64);
    let mem_stats = memory::stats();
    serial_println!(
        "[MEM] 64 KiB Bump Allocator initialized: used={} free={} total={}",
        mem_stats.used,
        mem_stats.free,
        mem_stats.total
    );

    // 5. Initialize Dijkstra's Banker's Algorithm & Verify Safety
    let banker_status = deadlock::status();
    if banker_status.is_safe {
        let seq = banker_status.safe_sequence;
        serial_println!(
            "[BANK] Banker's Algorithm: SAFE (seq: P{}, P{}, P{}, P{})",
            seq[0], seq[1], seq[2], seq[3]
        );
    } else {
        serial_println!("[BANK] Banker's Algorithm: WARNING - Unsafe initial state!");
    }

    // 6. Verify SpinLock primitive
    let test_lock = sync::SpinLock::new(0usize);
    {
        let mut guard = test_lock.lock();
        *guard += 1;
    }
    serial_println!("[SYNC] SpinLock verified: counter={}", *test_lock.lock());

    // 7. Register 4 tasks in MLFQ Scheduler
    {
        let mut sched = scheduler::SCHEDULER.lock();
        let _ = sched.add_task(Task::new(0, "shell", TaskClass::Interactive, 100));
        let _ = sched.add_task(Task::new(1, "logger", TaskClass::Interactive, 80));
        let _ = sched.add_task(Task::new(2, "worker", TaskClass::Compute, 150));
        let _ = sched.add_task(Task::new(3, "idle", TaskClass::Compute, 50));
    }
    serial_println!("[SCHED] Initialized MLFQ Scheduler: 4 tasks registered (shell, logger, worker, idle)");

    // 8. Initialize 64-bit IDT descriptor table
    interrupts::init_idt();
    serial_println!("[BOOT] 64-bit IDT loaded (DF, PF, GPF, Breakpoint, IRQ0, IRQ1).");

    // 9. Initialize PIC 8259 cascade remap
    pic::init();
    serial_println!("[BOOT] PIC 8259 cascade remapped to IRQ vectors 32..47.");

    // 10. Initialize PIT 8253 timer to 100 Hz (10ms tick interval)
    pit::init(100);
    serial_println!("[BOOT] PIT 8253 timer configured for 100 Hz (10ms tick interval).");

    // 11. Enable hardware interrupts
    x86_64::instructions::interrupts::enable();
    serial_println!("[BOOT] Hardware interrupts enabled (sti).");

    // 12. Run Animated Boot Splash Screen (Feature 20)
    splash::run();

    // 13. Initialize 80x25 TUI Multi-Panel Dashboard (Feature 21)
    ui::init();
    serial_println!("[BOOT] 80x25 TUI Multi-Panel Dashboard initialized.");

    // 14. Initialize Interactive Command Shell (Feature 22 & 23)
    shell::init();
    serial_println!("[BOOT] Interactive Command Shell ready.");

    // 15. Interactive Event Loop
    let mut last_ui_tick = pit::ticks();

    loop {
        // Drain incoming characters from PS/2 keyboard ring buffer
        while let Some(ch) = keyboard::pop_char() {
            shell::handle_char(ch);
        }

        // Periodic UI dashboard live update every 10 PIT ticks (100ms)
        let now = pit::ticks();
        if now.saturating_sub(last_ui_tick) >= 10 {
            last_ui_tick = now;
            ui::update_live_panels();
        }

        // Power-saving halt until next interrupt (IRQ0 / IRQ1)
        x86_64::instructions::hlt();
    }
}

/// Global panic handler for bare-metal kernel execution.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("[KERNEL PANIC] {}", info);
    vga_println!("[KERNEL PANIC] {}", info);
    loop {
        x86_64::instructions::hlt();
    }
}
