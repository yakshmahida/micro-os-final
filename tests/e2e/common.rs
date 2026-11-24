//! Common verification utilities and test contracts for Titanium Micro-OS.
//! CS2001 OS Capstone | Student: Yakshendrasinh R Mahida (USN: 25BTRCL288)

use std::path::{Path, PathBuf};

// Re-export core OS mechanisms directly from host library
pub use micro_os::task::{Task, TaskClass, TaskState};
pub use micro_os::scheduler::{MlfqScheduler, ScheduleEvent};
pub use micro_os::sync::{SpinLock, SpinLockGuard};
pub use micro_os::memory::{BumpAllocator, AllocatorStats};
pub use micro_os::deadlock::{BankersAlgorithm, BankerStatus};

// ============================================================================
// 1. Filesystem & Binary Inspection Utilities (Surface A)
// ============================================================================

/// Sector size for standard ISO 9660 filesystem (2048 bytes).
pub const ISO_SECTOR_SIZE: usize = 2048;

/// PVD starts at sector 16 (byte offset 32768).
pub const ISO_PVD_SECTOR: usize = 16;
pub const ISO_PVD_OFFSET: usize = ISO_PVD_SECTOR * ISO_SECTOR_SIZE;

/// El Torito Boot Record starts at sector 17 (byte offset 34816).
pub const EL_TORITO_SECTOR: usize = 17;
pub const EL_TORITO_OFFSET: usize = EL_TORITO_SECTOR * ISO_SECTOR_SIZE;

/// Default Boot Catalog sector (sector 19 = offset 38912).
pub const BOOT_CATALOG_SECTOR: usize = 19;
pub const BOOT_CATALOG_OFFSET: usize = BOOT_CATALOG_SECTOR * ISO_SECTOR_SIZE;

/// Verifies ISO 9660 Primary Volume Descriptor (PVD) at sector 16.
pub fn verify_iso_pvd_sector16(data: &[u8]) -> Result<(), String> {
    if data.len() < ISO_PVD_OFFSET + ISO_SECTOR_SIZE {
        return Err(format!(
            "ISO data too small: {} bytes, expected at least {} bytes",
            data.len(),
            ISO_PVD_OFFSET + ISO_SECTOR_SIZE
        ));
    }
    let pvd = &data[ISO_PVD_OFFSET..ISO_PVD_OFFSET + ISO_SECTOR_SIZE];

    // Byte 0: Type code 1 for Primary Volume Descriptor
    if pvd[0] != 0x01 {
        return Err(format!("PVD Type Code invalid: expected 0x01, found 0x{:02X}", pvd[0]));
    }

    // Bytes 1..6: Standard Identifier "CD001"
    if &pvd[1..6] != b"CD001" {
        return Err(format!(
            "PVD Identifier invalid: expected 'CD001', found {:?}",
            &pvd[1..6]
        ));
    }

    // Byte 6: Version must be 1
    if pvd[6] != 0x01 {
        return Err(format!("PVD Version invalid: expected 1, found {}", pvd[6]));
    }

    Ok(())
}

/// Verifies El Torito Boot Record at sector 17 and returns the boot catalog sector.
pub fn verify_el_torito_sector17(data: &[u8]) -> Result<u32, String> {
    if data.len() < EL_TORITO_OFFSET + ISO_SECTOR_SIZE {
        return Err(format!(
            "ISO data too small for El Torito sector 17: {} bytes",
            data.len()
        ));
    }
    let br = &data[EL_TORITO_OFFSET..EL_TORITO_OFFSET + ISO_SECTOR_SIZE];

    // Byte 0: Boot Record Indicator (0x00)
    if br[0] != 0x00 {
        return Err(format!("Boot Record Indicator invalid: expected 0x00, found 0x{:02X}", br[0]));
    }

    // Bytes 1..6: "CD001"
    if &br[1..6] != b"CD001" {
        return Err(format!("Boot Record CD identifier invalid: {:?}", &br[1..6]));
    }

    // Byte 6: Version (0x01)
    if br[6] != 0x01 {
        return Err(format!("Boot Record Version invalid: {}", br[6]));
    }

    // Bytes 7..39: Boot System Identifier "EL TORITO SPECIFICATION"
    let sys_id = &br[7..39];
    let sys_id_str = String::from_utf8_lossy(sys_id);
    if !sys_id_str.starts_with("EL TORITO SPECIFICATION") {
        return Err(format!("Invalid boot system ID: '{}'", sys_id_str.trim()));
    }

    // Bytes 71..75: 32-bit absolute sector of Boot Catalog (little-endian)
    let catalog_sector = u32::from_le_bytes([br[71], br[72], br[73], br[74]]);
    Ok(catalog_sector)
}

/// Verifies El Torito Boot Catalog at the specified sector.
pub fn verify_el_torito_boot_catalog(data: &[u8], catalog_sector: u32) -> Result<(), String> {
    let offset = (catalog_sector as usize) * ISO_SECTOR_SIZE;
    if data.len() < offset + ISO_SECTOR_SIZE {
        return Err(format!(
            "ISO data too small for Boot Catalog at sector {}: {} bytes",
            catalog_sector, data.len()
        ));
    }
    let cat = &data[offset..offset + ISO_SECTOR_SIZE];

    // Validation Entry (32 bytes):
    // Byte 0: Header ID (0x01)
    if cat[0] != 0x01 {
        return Err(format!("Validation Entry Header ID invalid: expected 0x01, found 0x{:02X}", cat[0]));
    }
    // Byte 1: Platform ID (0x00 for 80x86)
    if cat[1] != 0x00 {
        return Err(format!("Validation Entry Platform ID invalid: expected 0x00 (x86), found 0x{:02X}", cat[1]));
    }
    // Bytes 30..32: Key bytes 0x55, 0xAA
    if cat[30] != 0x55 || cat[31] != 0xAA {
        return Err(format!("Validation Entry Key invalid: expected [0x55, 0xAA], found [0x{:02X}, 0x{:02X}]", cat[30], cat[31]));
    }

    // Verify Checksum (16 words of 16-bit values must sum to 0 mod 0x10000)
    let mut sum: u32 = 0;
    for i in 0..16 {
        let word = u16::from_le_bytes([cat[i * 2], cat[i * 2 + 1]]);
        sum = sum.wrapping_add(word as u32);
    }
    if (sum & 0xFFFF) != 0 {
        return Err(format!("Boot Catalog checksum verification failed: sum = 0x{:04X}", sum & 0xFFFF));
    }

    // Initial / Default Entry (offset 32):
    let entry = &cat[32..64];
    // Byte 0: Boot indicator (0x88 for bootable, 0x00 for not bootable)
    if entry[0] != 0x88 {
        return Err(format!("Initial Entry Boot Indicator invalid: expected 0x88, found 0x{:02X}", entry[0]));
    }
    // Byte 1: Boot media type (0x00 for No Emulation)
    if entry[1] != 0x00 {
        return Err(format!("Initial Entry Media Type invalid: expected 0x00 (No Emulation), found 0x{:02X}", entry[1]));
    }

    Ok(())
}

/// Generates a valid spec-compliant minimal ISO 9660 + El Torito buffer.
pub fn generate_spec_compliant_iso() -> Vec<u8> {
    let total_sectors = 32;
    let mut iso = vec![0u8; total_sectors * ISO_SECTOR_SIZE];

    // Sector 16: Primary Volume Descriptor
    let pvd = &mut iso[ISO_PVD_OFFSET..ISO_PVD_OFFSET + ISO_SECTOR_SIZE];
    pvd[0] = 0x01; // PVD
    pvd[1..6].copy_from_slice(b"CD001");
    pvd[6] = 0x01; // Version
    let sys_id = b"TITANIUM_MICRO_OS_X86_64        ";
    pvd[8..40].copy_from_slice(sys_id);
    let vol_id = b"TITANIUM_OS                     ";
    pvd[40..72].copy_from_slice(vol_id);
    pvd[128..130].copy_from_slice(&2048u16.to_le_bytes());
    pvd[130..132].copy_from_slice(&2048u16.to_be_bytes());

    // Sector 17: El Torito Boot Record
    let br = &mut iso[EL_TORITO_OFFSET..EL_TORITO_OFFSET + ISO_SECTOR_SIZE];
    br[0] = 0x00; // Boot record indicator
    br[1..6].copy_from_slice(b"CD001");
    br[6] = 0x01; // Version
    let torito_id = b"EL TORITO SPECIFICATION\0\0\0\0\0\0\0\0\0";
    br[7..39].copy_from_slice(torito_id);
    br[71..75].copy_from_slice(&(BOOT_CATALOG_SECTOR as u32).to_le_bytes());

    // Sector 19: Boot Catalog
    let cat = &mut iso[BOOT_CATALOG_OFFSET..BOOT_CATALOG_OFFSET + ISO_SECTOR_SIZE];
    cat[0] = 0x01; // Header ID
    cat[1] = 0x00; // x86
    cat[30] = 0x55;
    cat[31] = 0xAA;
    let mut sum: u32 = 0;
    for i in 0..16 {
        if i != 14 {
            let word = u16::from_le_bytes([cat[i * 2], cat[i * 2 + 1]]);
            sum = sum.wrapping_add(word as u32);
        }
    }
    let checksum = (0x10000u32.wrapping_sub(sum & 0xFFFF)) as u16;
    cat[28..30].copy_from_slice(&checksum.to_le_bytes());

    // Initial / Default Entry at offset 32:
    cat[32] = 0x88; // Bootable
    cat[33] = 0x00; // No Emulation
    cat[34..36].copy_from_slice(&0x0000u16.to_le_bytes()); // Load segment 0
    cat[36] = 0x00; // System type
    cat[38..40].copy_from_slice(&4u16.to_le_bytes()); // Sector count (4 sectors = 8 KiB)
    cat[40..44].copy_from_slice(&20u32.to_le_bytes()); // Starting RBA (sector 20)

    iso
}

/// Verifies VirtualBox VDI Disk Header.
pub fn verify_vdi_header(data: &[u8]) -> Result<(), String> {
    if data.len() < 0x190 {
        return Err(format!("VDI data too small: {} bytes, expected >= 400 bytes", data.len()));
    }

    let desc = String::from_utf8_lossy(&data[0..64]);
    if !desc.contains("Oracle") || !desc.contains("Disk Image") {
        return Err(format!("VDI magic description missing, found: '{}'", desc.trim()));
    }

    let signature = u32::from_le_bytes([data[0x40], data[0x41], data[0x42], data[0x43]]);
    if signature != 0x7f10da7f && signature != 0xbeda107f {
        return Err(format!("VDI signature invalid: expected 0x7F10DA7F or 0xBEDA107F, found 0x{:08X}", signature));
    }

    let version = u32::from_le_bytes([data[0x44], data[0x45], data[0x46], data[0x47]]);
    if version != 0x00010001 {
        return Err(format!("VDI version invalid: expected 0x00010001, found 0x{:08X}", version));
    }

    let header_size = u32::from_le_bytes([data[0x48], data[0x49], data[0x4A], data[0x4B]]);
    if header_size < 0x190 {
        return Err(format!("VDI header size invalid: expected >= 400, found {}", header_size));
    }

    Ok(())
}

/// Generates a valid spec-compliant VDI header buffer.
pub fn generate_spec_compliant_vdi() -> Vec<u8> {
    let mut vdi = vec![0u8; 512];
    let magic_desc = b"<<< Oracle VM VirtualBox Disk Image >>>\n";
    vdi[0..magic_desc.len()].copy_from_slice(magic_desc);
    vdi[0x40..0x44].copy_from_slice(&0x7f10da7fu32.to_le_bytes()); // Signature
    vdi[0x44..0x48].copy_from_slice(&0x00010001u32.to_le_bytes()); // Version 1.1
    vdi[0x48..0x4C].copy_from_slice(&0x0190u32.to_le_bytes());     // Header size = 400
    vdi[0x4C..0x50].copy_from_slice(&1u32.to_le_bytes());          // Image type 1 (dynamic)
    vdi
}

/// Surface A: Validates kernel binary slice invariants.
pub fn validate_kernel_binary_slice(data: &[u8]) -> Result<(), &'static str> {
    if data.is_empty() {
        return Err("KernelZeroLength");
    }
    if data.len() < 512 {
        return Err("KernelTooSmall");
    }
    Ok(())
}

/// Surface A: Validates toolchain channel specification.
pub fn validate_toolchain_channel(channel: &str) -> Result<(), &'static str> {
    if !channel.contains("nightly") {
        return Err("RequiresNightlyCompiler");
    }
    Ok(())
}

/// Surface A: Validates existence of kernel binary path.
pub fn check_kernel_binary_path(path: &Path) -> Result<PathBuf, &'static str> {
    match path.try_exists() {
        Ok(true) => Ok(path.to_path_buf()),
        _ => Err("KernelNotFound"),
    }
}

/// Surface A: Validates VBoxManage availability status handling.
pub fn evaluate_vboxmanage_status(installed: bool) -> Result<(), (i32, &'static str)> {
    if installed {
        Ok(())
    } else {
        Err((3, "VirtualBox automation requires VBoxManage"))
    }
}

// ============================================================================
// 2. Hardware Register & Encoding Utilities (Surface D)
// ============================================================================

/// Computes VGA 80x25 text-mode color attribute byte: `(bg << 4) | (fg & 0x0F)`.
pub const fn vga_attribute_byte(fg: u8, bg: u8) -> u8 {
    ((bg & 0x0F) << 4) | (fg & 0x0F)
}

/// Maps Unicode characters to standard IBM CP437 box-drawing codes.
pub fn cp437_box_drawing_map(ch: char) -> Option<u8> {
    match ch {
        '─' => Some(0xC4), // Single horizontal
        '│' => Some(0xB3), // Single vertical
        '┌' => Some(0xDA), // Single top-left
        '┐' => Some(0xBF), // Single top-right
        '└' => Some(0xC0), // Single bottom-left
        '┘' => Some(0xD9), // Single bottom-right
        '├' => Some(0xC3), // Single left tee
        '┤' => Some(0xB4), // Single right tee
        '┬' => Some(0xC2), // Single top tee
        '┴' => Some(0xC1), // Single bottom tee
        '┼' => Some(0xC5), // Single cross
        '═' => Some(0xCD), // Double horizontal
        '║' => Some(0xBA), // Double vertical
        '╔' => Some(0xC9), // Double top-left
        '╗' => Some(0xBB), // Double top-right
        '╚' => Some(0xC8), // Double bottom-left
        '╝' => Some(0xBC), // Double bottom-right
        '■' => Some(0xFE), // Filled square block
        _ => None,
    }
}

/// Decodes PS/2 Set 1 make scancodes into ASCII characters.
pub fn ps2_scancode_to_char(scancode: u8) -> Option<char> {
    // Ignore Break codes (>= 0x80) and extended prefix (0xE0)
    if scancode >= 0x80 || scancode == 0xE0 {
        return None;
    }
    match scancode {
        0x1E => Some('a'),
        0x30 => Some('b'),
        0x2E => Some('c'),
        0x20 => Some('d'),
        0x12 => Some('e'),
        0x21 => Some('f'),
        0x22 => Some('g'),
        0x23 => Some('h'),
        0x17 => Some('i'),
        0x24 => Some('j'),
        0x25 => Some('k'),
        0x26 => Some('l'),
        0x32 => Some('m'),
        0x31 => Some('n'),
        0x18 => Some('o'),
        0x19 => Some('p'),
        0x10 => Some('q'),
        0x13 => Some('r'),
        0x1F => Some('s'),
        0x14 => Some('t'),
        0x16 => Some('u'),
        0x2F => Some('v'),
        0x11 => Some('w'),
        0x2D => Some('x'),
        0x15 => Some('y'),
        0x2C => Some('z'),
        0x02 => Some('1'),
        0x03 => Some('2'),
        0x04 => Some('3'),
        0x05 => Some('4'),
        0x06 => Some('5'),
        0x07 => Some('6'),
        0x08 => Some('7'),
        0x09 => Some('8'),
        0x0A => Some('9'),
        0x0B => Some('0'),
        0x39 => Some(' '),
        0x1C => Some('\n'),
        0x0E => Some('\x08'), // Backspace
        _ => None,
    }
}

/// 64-bit IDT Gate Descriptor layout (16 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
pub struct IdtGateDescriptor {
    pub offset_low: u16,
    pub selector: u16,
    pub ist: u8,
    pub type_attr: u8,
    pub offset_mid: u16,
    pub offset_high: u32,
    pub reserved: u32,
}

pub fn encode_interrupt_gate(handler_addr: u64, selector: u16, gate_type: u8) -> IdtGateDescriptor {
    IdtGateDescriptor {
        offset_low: (handler_addr & 0xFFFF) as u16,
        selector,
        ist: 0,
        type_attr: gate_type,
        offset_mid: ((handler_addr >> 16) & 0xFFFF) as u16,
        offset_high: ((handler_addr >> 32) & 0xFFFFFFFF) as u32,
        reserved: 0,
    }
}

/// PIC 8259 Cascade Remapping EOI Routing Contract.
/// Returns (master_eoi, slave_eoi).
pub fn pic_eoi_routing(irq: u8) -> (bool, bool) {
    if irq >= 16 {
        (false, false)
    } else if irq >= 8 {
        (true, true)
    } else {
        (true, false)
    }
}

/// PIC 8259 Spurious Interrupt EOI Handling Contract.
/// Returns (master_eoi, slave_eoi).
pub fn pic_handle_spurious(irq: u8, isr_bit_set: bool) -> (bool, bool) {
    match irq {
        7 => {
            if isr_bit_set {
                (true, false) // Real IRQ 7
            } else {
                (false, false) // Spurious IRQ 7: NO EOI
            }
        }
        15 => {
            if isr_bit_set {
                (true, true) // Real IRQ 15
            } else {
                (true, false) // Spurious IRQ 15: Master EOI only, NO Slave EOI
            }
        }
        _ => pic_eoi_routing(irq),
    }
}

/// VGA 80x25 Screen Coordinate Buffer Offset Calculator.
pub fn vga_cell_offset(col: usize, row: usize) -> Result<usize, &'static str> {
    if col >= 80 || row >= 25 {
        return Err("OutOfBounds");
    }
    Ok((row * 80 + col) * 2)
}

/// UART 16550 Line Status Register (LSR) Transmitter Ready Bitmask.
pub fn uart_can_transmit(lsr: u8) -> bool {
    (lsr & 0x20) != 0 // Bit 5 is Transmitter Holding Register Empty (THRE)
}

/// Reusable PS/2 Scancode Ring Buffer Model.
pub struct ScancodeRingBuffer<const N: usize> {
    buf: [u8; N],
    head: usize,
    tail: usize,
    count: usize,
}

impl<const N: usize> ScancodeRingBuffer<N> {
    pub fn new() -> Self {
        Self {
            buf: [0; N],
            head: 0,
            tail: 0,
            count: 0,
        }
    }
    pub fn push(&mut self, val: u8) -> bool {
        if self.count >= N {
            false
        } else {
            self.buf[self.tail] = val;
            self.tail = (self.tail + 1) % N;
            self.count += 1;
            true
        }
    }
    pub fn pop(&mut self) -> Option<u8> {
        if self.count == 0 {
            None
        } else {
            let val = self.buf[self.head];
            self.head = (self.head + 1) % N;
            self.count -= 1;
            Some(val)
        }
    }
    pub fn count(&self) -> usize {
        self.count
    }
}

impl<const N: usize> Default for ScancodeRingBuffer<N> {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 3. Serial Trace Pipeline & Parser (Surface E)
// ============================================================================

/// Canonical VirtualBox script reference for verification across tests.
pub const RUN_VBOX_SCRIPT_REF: &str = r#"#!/usr/bin/env bash
set -e

VM_NAME="TitaniumMicroOS"
ISO_PATH="dist/micro-os.iso"
LOG_DIR="traces"
LOG_FILE="${LOG_DIR}/qemu-boot.log"

# Clean poweroff and unregister if pre-existing
if command -v VBoxManage &> /dev/null; then
    VBoxManage controlvm "${VM_NAME}" poweroff 2>/dev/null || true
    VBoxManage unregistervm "${VM_NAME}" --delete 2>/dev/null || true

    mkdir -p "${LOG_DIR}"

    if [ ! -f "${ISO_PATH}" ]; then
        echo "Error: ISO not found at ${ISO_PATH}"
        exit 1
    fi

    # Create and configure VM
    VBoxManage createvm --name "${VM_NAME}" --ostype "Other" --register
    VBoxManage modifyvm "${VM_NAME}" --memory 128 --firmware bios
    VBoxManage setextradata "${VM_NAME}" "VBoxInternal2/EnableX86OnArm" 1 2>/dev/null || true

    # Storage controller & ISO attachment
    VBoxManage storagectl "${VM_NAME}" --name "IDE Controller" --add ide 2>/dev/null || true
    VBoxManage storageattach "${VM_NAME}" --storagectl "IDE Controller" --port 0 --device 0 --type dvddrive --medium "${ISO_PATH}"

    # Serial port redirection to trace log
    VBoxManage modifyvm "${VM_NAME}" --uart1 0x3F8 4 --uartmode1 file "${LOG_FILE}"

    # Autonomous GUI Launch
    VBoxManage startvm "${VM_NAME}" --type gui
else
    echo "Warning: VBoxManage not installed on host."
    exit 3
fi
"#;

/// Parsed serial scheduler trace record.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceRecord {
    pub tick: usize,
    pub task_id: usize,
    pub name: String,
    pub queue: usize,
    pub state: String,
    pub exec_time: usize,
    pub wait_time: usize,
    pub yield_time: usize,
    pub burst_ratio: f64,
}

/// Parses a single line from the serial execution trace log.
/// Schema: `[SCHED] tick=(\d+) task=(\d+) \((\w+)\) Q=(\d).*Θ=([0-9.]+)`
pub fn parse_trace_line(line: &str) -> Option<TraceRecord> {
    if !line.starts_with("[SCHED]") {
        return None;
    }
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 5 {
        return None;
    }

    let mut tick = None;
    let mut task_id = None;
    let mut name = String::new();
    let mut queue = None;
    let mut state = "RUNNING".to_string();
    let mut exec_time = 0;
    let mut wait_time = 0;
    let mut yield_time = 0;
    let mut burst_ratio = None;

    for part in parts {
        if let Some(t_str) = part.strip_prefix("tick=") {
            tick = t_str.parse::<usize>().ok();
        } else if let Some(id_str) = part.strip_prefix("task=") {
            task_id = id_str.parse::<usize>().ok();
        } else if part.starts_with('(') && part.ends_with(')') {
            name = part[1..part.len() - 1].to_string();
        } else if let Some(q_str) = part.strip_prefix("Q=") {
            queue = q_str.parse::<usize>().ok();
        } else if let Some(s_str) = part.strip_prefix("state=") {
            state = s_str.to_string();
        } else if let Some(e_str) = part.strip_prefix("exec=") {
            let val = e_str.trim_end_matches("ms");
            exec_time = val.parse::<usize>().unwrap_or(0);
        } else if let Some(w_str) = part.strip_prefix("wait=") {
            let val = w_str.trim_end_matches("ms");
            wait_time = val.parse::<usize>().unwrap_or(0);
        } else if let Some(y_str) = part.strip_prefix("yield=") {
            let val = y_str.trim_end_matches("ms");
            yield_time = val.parse::<usize>().unwrap_or(0);
        } else if let Some(th_str) = part.strip_prefix("Θ=") {
            burst_ratio = th_str.parse::<f64>().ok();
        } else if let Some(th_str) = part.strip_prefix("theta=") {
            burst_ratio = th_str.parse::<f64>().ok();
        }
    }

    if let (Some(t), Some(id), Some(q), Some(br)) = (tick, task_id, queue, burst_ratio) {
        Some(TraceRecord {
            tick: t,
            task_id: id,
            name,
            queue: q,
            state,
            exec_time,
            wait_time,
            yield_time,
            burst_ratio: br,
        })
    } else {
        None
    }
}

/// Formats a TraceRecord into the standard serial trace line format.
pub fn format_trace_line(record: &TraceRecord) -> String {
    format!(
        "[SCHED] tick={} task={} ({}) Q={} state={} exec={}ms wait={}ms yield={}ms Θ={:.4}",
        record.tick,
        record.task_id,
        record.name,
        record.queue,
        record.state,
        record.exec_time,
        record.wait_time,
        record.yield_time,
        record.burst_ratio
    )
}

// ============================================================================
// 4. Command & TUI Contract Evaluation (Surface C)
// ============================================================================

/// Renders simulated animated boot splash frame.
pub fn render_splash_frame(progress_pct: usize) -> String {
    let bar_width = 30;
    let filled = (progress_pct * bar_width) / 100;
    let bar: String = "■".repeat(filled) + &"-".repeat(bar_width - filled);
    format!(
        "╔══════════════════════════════════════════════════════════════════════════════╗\n\
         ║                        TITANIUM MICRO-OS v0.1.0                              ║\n\
         ║           CS2001 OS Capstone | Yakshendrasinh R Mahida (25BTRCL288)             ║\n\
         ║                                                                              ║\n\
         ║                   Booting Kernel... [{}] {:3}%                  ║\n\
         ╚══════════════════════════════════════════════════════════════════════════════╝",
        bar, progress_pct
    )
}

/// Panel layout specification for 80x25 dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TuiPanel {
    pub title: &'static str,
    pub col: usize,
    pub row: usize,
    pub width: usize,
    pub height: usize,
}

pub const STANDARD_DASHBOARD_PANELS: [TuiPanel; 5] = [
    TuiPanel { title: "SYSTEM INFO", col: 0, row: 0, width: 40, height: 7 },
    TuiPanel { title: "TASK MLFQ", col: 40, row: 0, width: 40, height: 7 },
    TuiPanel { title: "KERNEL LOG", col: 0, row: 7, width: 40, height: 10 },
    TuiPanel { title: "MEM / BANKER", col: 40, row: 7, width: 40, height: 10 },
    TuiPanel { title: "SHELL", col: 0, row: 17, width: 80, height: 8 },
];

/// Shell Command Dispatcher & Dynamic Verification Engine.
pub struct ShellDispatcher {
    pub history: Vec<String>,
    pub output_buffer: Vec<String>,
    pub allocator: BumpAllocator,
    pub scheduler: MlfqScheduler,
    pub banker: BankersAlgorithm,
    pub tasks: Vec<Task>,
}

pub type OpaqueShellDispatcher = ShellDispatcher;

impl Default for ShellDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl ShellDispatcher {
    pub fn new() -> Self {
        let mut allocator = BumpAllocator::new();
        // Pre-allocate 16 KiB so default stats match standard OS baseline
        allocator.allocate(16384, 64);

        let mut scheduler = MlfqScheduler::new();
        let tasks = vec![
            Task::new(1, "shell", TaskClass::Interactive, 100),
            Task::new(2, "logger", TaskClass::Interactive, 80),
            Task::new(3, "worker", TaskClass::Compute, 150),
            Task::new(4, "idle", TaskClass::Compute, 50),
        ];
        for t in &tasks {
            scheduler.add_task(t.clone());
        }

        let mut banker = BankersAlgorithm::new(5, 3);
        banker.set_available(&[3, 3, 2]);
        banker.set_allocation(0, &[0, 1, 0]);
        banker.set_max(0, &[7, 5, 3]);
        banker.set_allocation(1, &[2, 0, 0]);
        banker.set_max(1, &[3, 2, 2]);
        banker.set_allocation(2, &[3, 0, 2]);
        banker.set_max(2, &[9, 0, 2]);
        banker.set_allocation(3, &[2, 1, 1]);
        banker.set_max(3, &[2, 2, 2]);
        banker.set_allocation(4, &[0, 0, 2]);
        banker.set_max(4, &[4, 3, 3]);

        Self {
            history: Vec::new(),
            output_buffer: Vec::new(),
            allocator,
            scheduler,
            banker,
            tasks,
        }
    }

    /// Dispatches a raw command line with length boundary protection (128 chars).
    pub fn execute(&mut self, input: &str) -> String {
        let sanitized = if input.len() > 128 {
            &input[..128]
        } else {
            input
        };

        let trimmed = sanitized.trim();
        if trimmed.is_empty() {
            return String::new();
        }

        self.history.push(trimmed.to_string());
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        let cmd = tokens[0];

        let result = match cmd {
            "help" => {
                let cmds = [
                    ("help", "Display this list of commands"),
                    ("tasks", "List registered tasks and execution states"),
                    ("sched", "Show 3-queue MLFQ status & burst ratios"),
                    ("mem", "Display 64 KiB Bump Allocator memory usage"),
                    ("deadlock", "Show Dijkstra's Banker's Algorithm status"),
                    ("info", "System metadata and student USN"),
                    ("reset", "Reset shell state and telemetry"),
                    ("clear", "Clear shell output buffer"),
                    ("web", "Display simulated network portal & status viewer"),
                ];
                let mut out = String::from("Supported commands:\n");
                for (name, desc) in &cmds {
                    out.push_str(&format!("  {:<9} - {}\n", name, desc));
                }
                out.trim_end().to_string()
            }
            "tasks" => {
                let mut out = String::from("PID  NAME      CLASS        STATE       BURST_RATIO  QUEUE\n");
                for t in &self.tasks {
                    let class_str = match t.class {
                        TaskClass::Interactive => "Interactive",
                        TaskClass::Compute => "Compute",
                    };
                    let state_str = match t.state {
                        TaskState::Ready => "READY",
                        TaskState::Running => "RUNNING",
                        TaskState::Blocked => "BLOCKED",
                        TaskState::Terminated => "TERMINATED",
                    };
                    out.push_str(&format!(
                        "  {:<2} {:<9} {:<12} {:<11} {:<12.4} Q{}\n",
                        t.id, t.name, class_str, state_str, t.burst_ratio(), t.queue
                    ));
                }
                out.trim_end().to_string()
            }
            "sched" => {
                format!(
                    "MLFQ Scheduler Status:\n  Queues: Q0 (quantum={}ms), Q1 (quantum={}ms), Q2 (quantum={}ms)\n  Boost Interval: {}ms (promotes all ready tasks to Q0)\n  Classification Threshold: Θ < 0.20 (Interactive) vs Θ >= 0.20 (Compute)\n  Active Tasks: {} | Context Switches: {}",
                    self.scheduler.quantums_ms[0],
                    self.scheduler.quantums_ms[1],
                    self.scheduler.quantums_ms[2],
                    self.scheduler.boost_interval,
                    self.scheduler.total_tasks(),
                    self.scheduler.switches
                )
            }
            "mem" => {
                let stats = self.allocator.stats();
                let pct = (stats.used as f64 / stats.total as f64) * 100.0;
                let free_pct = (stats.free as f64 / stats.total as f64) * 100.0;
                format!(
                    "Bump Allocator Status:\n  Total Capacity: {} bytes ({} KiB)\n  Allocated:      {} bytes ({:.1}%)\n  Remaining Free: {} bytes ({:.1}%)\n  Alignment:      Power-of-2 enforced (1, 2, 4, 8, 16, 64 B)",
                    stats.total,
                    stats.total / 1024,
                    stats.used,
                    pct,
                    stats.free,
                    free_pct
                )
            }
            "deadlock" => {
                let status = self.banker.status();
                let state_str = if status.is_safe { "SAFE" } else { "UNSAFE" };
                let seq_str = status
                    .safe_sequence
                    .iter()
                    .map(|p| format!("P{}", p))
                    .collect::<Vec<_>>()
                    .join(" -> ");
                let avail_str = format!(
                    "[A: {}, B: {}, C: {}]",
                    status.available.first().unwrap_or(&0),
                    status.available.get(1).unwrap_or(&0),
                    status.available.get(2).unwrap_or(&0),
                );
                format!(
                    "Banker's Deadlock Avoidance:\n  System State:   {}\n  Available:      {}\n  Safe Sequence:  <{}>\n  Active Claims:  {} processes evaluated, 0 deadlock cycles detected",
                    state_str, avail_str, seq_str, self.banker.n
                )
            }
            "info" => {
                "Titanium Micro-OS v0.1.0 (x86_64 bare-metal)\nCS2001 Operating Systems Capstone Project\nStudent: Yakshendrasinh R Mahida | USN: 25BTRCL288\nTarget: x86_64-unknown-none | Bootloader: 0.11.17 BIOS".to_string()
            }
            "reset" => {
                self.history.clear();
                self.output_buffer.clear();
                self.allocator.reset();
                "System state and shell history reset successfully.".to_string()
            }
            "clear" => {
                self.output_buffer.clear();
                "\x1B[2J\x1B[H".to_string()
            }
            "web" => {
                let adapter = "VirtIO-Net (PCI 0x1AF4:0x1000)";
                let ip = "192.168.1.105";
                let mask = "255.255.255.0";
                let gateway = "192.168.1.1";
                let dns = "1.1.1.1, 8.8.8.8";
                let ping = 2.14;
                let http_code = 200;
                let banner = "Welcome to Titanium Micro-OS Embedded Web Status Viewer!";

                format!(
                    "╔══════════════════════════════════════════════════════════════════════════════╗\n\
                     ║                TITANIUM MICRO-OS SIMULATED NETWORK PORTAL                   ║\n\
                     ║                     CS2001 OS Capstone | USN: 25BTRCL288                     ║\n\
                     ╠══════════════════════════════════════════════════════════════════════════════╣\n\
                     ║ Network Adapter:   {:<53}║\n\
                     ║ IPv4 Address:      {:<53}║\n\
                     ║ Subnet Mask:       {:<53}║\n\
                     ║ Default Gateway:   {:<53}║\n\
                     ║ DNS Servers:       {:<53}║\n\
                     ║ Gateway Ping:      {:.2} ms (Round-Trip Latency, 0% Packet Loss)              ║\n\
                     ║ HTTP Request:      GET /portal/index.html -> {} OK (Content-Length: 4096)   ║\n\
                     ║ Portal Banner:     {:<53}║\n\
                     ╚══════════════════════════════════════════════════════════════════════════════╝",
                    adapter, ip, mask, gateway, dns, ping, http_code, banner
                )
            }
            unknown => {
                format!("Unknown command: '{}'. Type 'help' for available commands.", unknown)
            }
        };

        self.output_buffer.push(result.clone());
        result
    }

    pub fn history(&self) -> &[String] {
        &self.history
    }
}
