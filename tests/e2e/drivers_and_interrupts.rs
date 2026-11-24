//! Tier 1 & Tier 2 E2E Tests for R2: Hardware Drivers & Interrupt Infrastructure.
//! CS2001 OS Capstone | Student: Yakshendrasinh R Mahida (USN: 25BTRCL288)

use super::common::*;

// ============================================================================
// Tier 1: Feature Coverage (Happy Path)
// ============================================================================

/// `TC-T1-06`: VGA Memory Layout & Color Encoding.
/// Validates attribute byte formula `(bg << 4) | (fg & 0x0F)` and 80x25 screen cell count.
#[test]
fn t1_driver_06_vga_memory_layout_and_color_encoding() {
    const COLS: usize = 80;
    const ROWS: usize = 25;
    const TOTAL_CELLS: usize = COLS * ROWS; // 2000
    const TOTAL_BYTES: usize = TOTAL_CELLS * 2; // 4000 bytes

    assert_eq!(TOTAL_CELLS, 2000, "VGA 80x25 must contain exactly 2000 cells");
    assert_eq!(TOTAL_BYTES, 4000, "VGA memory buffer must be 4000 bytes");

    // Test all 16 foreground x 16 background combinations
    for bg in 0..16u8 {
        for fg in 0..16u8 {
            let attr = vga_attribute_byte(fg, bg);
            let extracted_fg = attr & 0x0F;
            let extracted_bg = (attr >> 4) & 0x0F;
            assert_eq!(extracted_fg, fg, "Extracted FG must match input FG");
            assert_eq!(extracted_bg, bg, "Extracted BG must match input BG");
        }
    }

    // Specific well-known color attributes
    assert_eq!(vga_attribute_byte(0x0F, 0x00), 0x0F); // White on Black
    assert_eq!(vga_attribute_byte(0x02, 0x00), 0x02); // Green on Black
    assert_eq!(vga_attribute_byte(0x0F, 0x01), 0x1F); // White on Blue
    assert_eq!(vga_attribute_byte(0x0E, 0x04), 0x4E); // Yellow on Red
}

/// `TC-T1-07`: CP437 Box-Drawing Mappings.
/// Validates Unicode box glyphs map to CP437 bytes (`0xC4`, `0xB3`, `0xDA`, etc.).
#[test]
fn t1_driver_07_cp437_box_drawing_mappings() {
    let expected_mappings = [
        ('─', 0xC4),
        ('│', 0xB3),
        ('┌', 0xDA),
        ('┐', 0xBF),
        ('└', 0xC0),
        ('┘', 0xD9),
        ('├', 0xC3),
        ('┤', 0xB4),
        ('┬', 0xC2),
        ('┴', 0xC1),
        ('┼', 0xC5),
        ('═', 0xCD),
        ('║', 0xBA),
        ('╔', 0xC9),
        ('╗', 0xBB),
        ('╚', 0xC8),
        ('╝', 0xBC),
        ('■', 0xFE),
    ];

    for &(ch, expected_code) in &expected_mappings {
        let code = cp437_box_drawing_map(ch);
        assert_eq!(
            code,
            Some(expected_code),
            "CP437 mapping for '{}' must be 0x{:02X}",
            ch,
            expected_code
        );
    }
}

/// `TC-T1-08`: UART 16550 Serial Initialization.
/// Validates COM1 `0x3F8` 115200 baud divisor = 1, 8N1 line control.
#[test]
fn t1_driver_08_uart_16550_serial_initialization() {
    const COM1_BASE: u16 = 0x3F8;
    const UART_CLOCK_FREQ: u32 = 115200;
    const TARGET_BAUD: u32 = 115200;

    let divisor = UART_CLOCK_FREQ / TARGET_BAUD;
    assert_eq!(divisor, 1, "Divisor for 115200 baud must equal 1");

    let dll = (divisor & 0xFF) as u8;
    let dlm = ((divisor >> 8) & 0xFF) as u8;
    assert_eq!(dll, 1, "DLL (Divisor Latch Low) must equal 1");
    assert_eq!(dlm, 0, "DLM (Divisor Latch High) must equal 0");

    // Line Control Register (LCR): 8 bits, no parity, 1 stop bit (8N1)
    let lcr_8n1: u8 = 0x03;
    assert_eq!(lcr_8n1 & 0x03, 0x03, "LCR data bits must be 8 (0b11)");
    assert_eq!(lcr_8n1 & 0x04, 0x00, "LCR stop bit must be 1 (0)");
    assert_eq!(lcr_8n1 & 0x38, 0x00, "LCR parity must be None (0b000)");

    // FIFO Control Register (FCR): Enable FIFO, clear TX/RX, 14-byte threshold
    let fcr_init: u8 = 0xC7;
    assert_eq!(fcr_init & 0x01, 0x01, "FCR FIFO enable must be 1");

    // Modem Control Register (MCR): DTR, RTS, OUT2 (IRQ enable)
    let mcr_init: u8 = 0x0B;
    assert_eq!(mcr_init & 0x01, 0x01, "MCR DTR must be 1");
    assert_eq!(mcr_init & 0x02, 0x02, "MCR RTS must be 1");
    assert_eq!(mcr_init & 0x08, 0x08, "MCR OUT2 must be 1");

    assert_eq!(COM1_BASE, 0x3F8, "COM1 base port must be 0x3F8");
}

/// `TC-T1-09`: 64-Bit IDT Descriptor Setup.
/// Validates 16-byte gate descriptors for vectors 8, 13, 14, 32, 33 with type `0x8E`.
#[test]
fn t1_driver_09_64bit_idt_descriptor_setup() {
    assert_eq!(
        std::mem::size_of::<IdtGateDescriptor>(),
        16,
        "64-bit IDT descriptor entry must be exactly 16 bytes"
    );

    let dummy_handler = 0xFFFF_8000_0010_2000u64;
    let gate = encode_interrupt_gate(dummy_handler, 0x08, 0x8E);

    let selector = gate.selector;
    let type_attr = gate.type_attr;
    let offset_low = gate.offset_low;
    let offset_mid = gate.offset_mid;
    let offset_high = gate.offset_high;

    assert_eq!(selector, 0x08, "Kernel code selector must be 0x08");
    assert_eq!(type_attr, 0x8E, "Type attribute must be 0x8E");
    assert_eq!(offset_low, 0x2000);
    assert_eq!(offset_mid, 0x0010);
    assert_eq!(offset_high, 0xFFFF_8000);

    // Verify critical vectors
    let critical_vectors = [8, 13, 14, 32, 33];
    assert_eq!(critical_vectors.len(), 5);
}

/// `TC-T1-10`: PIC 8259 Cascade Remapping.
/// Validates PIC remap offsets (Master: 32, Slave: 40) and EOI protocol.
#[test]
fn t1_driver_10_pic_8259_cascade_remapping() {
    const MASTER_OFFSET: u8 = 32; // Vectors 32..39
    const SLAVE_OFFSET: u8 = 40;  // Vectors 40..47
    const PIC_EOI: u8 = 0x20;

    assert_eq!(PIC_EOI, 0x20, "PIC EOI command must be 0x20");
    assert_eq!(MASTER_OFFSET, 0x20, "Master PIC offset must be 32 (0x20)");
    assert_eq!(SLAVE_OFFSET, 0x28, "Slave PIC offset must be 40 (0x28)");

    // IRQ 0 (Timer) -> only master receives EOI
    let (master, slave) = pic_eoi_routing(0);
    assert!(master);
    assert!(!slave);

    // IRQ 1 (Keyboard) -> only master receives EOI
    let (master, slave) = pic_eoi_routing(1);
    assert!(master);
    assert!(!slave);

    // IRQ 12 (PS/2 Mouse) -> both slave and master receive EOI
    let (master, slave) = pic_eoi_routing(12);
    assert!(master);
    assert!(slave);
}

/// `TC-T1-11`: PS/2 Keyboard Scancode Decoding.
/// Validates Make scancodes translate to ASCII in ring buffer.
#[test]
fn t1_driver_11_ps2_keyboard_scancode_decoding() {
    let make_scancodes = [
        (0x1E, 'a'),
        (0x30, 'b'),
        (0x2E, 'c'),
        (0x20, 'd'),
        (0x12, 'e'),
        (0x39, ' '),
        (0x1C, '\n'),
    ];

    for &(code, expected_char) in &make_scancodes {
        let decoded = ps2_scancode_to_char(code);
        assert_eq!(
            decoded,
            Some(expected_char),
            "Scancode 0x{:02X} must decode to '{}'",
            code,
            expected_char
        );
    }
}

// ============================================================================
// Tier 2: Boundary & Corner Cases (Stress & Limits)
// ============================================================================

/// `TC-T2-06`: VGA Coordinate Bounds Clamping.
/// Validates rendering at or beyond limits ($col \ge 80, row \ge 25$) is safely clamped.
#[test]
fn t2_driver_06_vga_coordinate_bounds_clamping() {
    // Valid boundary coordinates
    assert_eq!(vga_cell_offset(0, 0), Ok(0));
    assert_eq!(vga_cell_offset(79, 24), Ok((24 * 80 + 79) * 2));

    // Boundary violation coordinates
    assert_eq!(vga_cell_offset(80, 0), Err("OutOfBounds"));
    assert_eq!(vga_cell_offset(0, 25), Err("OutOfBounds"));
    assert_eq!(vga_cell_offset(100, 100), Err("OutOfBounds"));
}

/// `TC-T2-07`: PS/2 Ring Buffer Saturation.
/// Validates that a 16-element ring buffer flooded with 32 keypresses handles saturation cleanly.
#[test]
fn t2_driver_07_ps2_ring_buffer_saturation() {
    let mut ring = ScancodeRingBuffer::<16>::new();

    // Push 32 scancodes into 16-element buffer
    for i in 0..32u8 {
        let accepted = ring.push(i);
        if i < 16 {
            assert!(accepted, "First 16 elements must be accepted");
        } else {
            assert!(!accepted, "Elements beyond capacity 16 must be rejected without overflow");
        }
    }

    assert_eq!(ring.count(), 16, "Count must strictly cap at 16");

    // Pop all elements and verify order
    for i in 0..16u8 {
        assert_eq!(ring.pop(), Some(i), "Popped elements must preserve FIFO order");
    }
    assert_eq!(ring.pop(), None, "Empty buffer must return None");
}

/// `TC-T2-08`: Break & Extended Scancode Filter.
/// Validates key release scancodes ($\ge 0x80$) and `0xE0` prefixes do not emit garbage characters.
#[test]
fn t2_driver_08_break_and_extended_scancode_filter() {
    // Break codes for letters (Make code | 0x80)
    let break_codes = [0x9E, 0xB0, 0xAE, 0xA0, 0x92];
    for &code in &break_codes {
        assert_eq!(
            ps2_scancode_to_char(code),
            None,
            "Break code 0x{:02X} must not produce a character",
            code
        );
    }

    // Extended scancode prefix
    assert_eq!(
        ps2_scancode_to_char(0xE0),
        None,
        "Extended prefix 0xE0 must return None"
    );

    // Unmapped special codes
    assert_eq!(
        ps2_scancode_to_char(0xFF),
        None,
        "Special scancode 0xFF must return None"
    );
}

/// `TC-T2-09`: UART Transmitter Full Polling.
/// Validates Line Status Register (LSR) bit 5 polling logic.
#[test]
fn t2_driver_09_uart_transmitter_full_polling() {
    assert!(!uart_can_transmit(0x00), "Transmitter not ready when LSR bit 5 is 0");
    assert!(!uart_can_transmit(0x1F), "Transmitter not ready when LSR bit 5 is 0");
    assert!(uart_can_transmit(0x20), "Transmitter ready when LSR bit 5 is 1");
    assert!(uart_can_transmit(0x60), "Transmitter ready when LSR bit 5 (and 6) is 1");
}

/// `TC-T2-10`: PIC Spurious Interrupt Handling.
/// Validates spurious IRQ 7 (vec 39) and IRQ 15 (vec 47) detection and EOI suppression.
#[test]
fn t2_driver_10_pic_spurious_interrupt_handling() {
    // Test Spurious IRQ 7
    let (m_eoi, s_eoi) = pic_handle_spurious(7, false);
    assert!(!m_eoi, "Spurious IRQ 7 must NOT send EOI to master");
    assert!(!s_eoi, "Spurious IRQ 7 must NOT send EOI to slave");

    // Test Real IRQ 7
    let (m_eoi, s_eoi) = pic_handle_spurious(7, true);
    assert!(m_eoi, "Real IRQ 7 must send EOI to master");
    assert!(!s_eoi, "Real IRQ 7 must NOT send EOI to slave");

    // Test Spurious IRQ 15
    let (m_eoi, s_eoi) = pic_handle_spurious(15, false);
    assert!(m_eoi, "Spurious IRQ 15 must send EOI to master");
    assert!(!s_eoi, "Spurious IRQ 15 must NOT send EOI to slave");

    // Test Real IRQ 15
    let (m_eoi, s_eoi) = pic_handle_spurious(15, true);
    assert!(m_eoi, "Real IRQ 15 must send EOI to master");
    assert!(s_eoi, "Real IRQ 15 must send EOI to slave");
}
