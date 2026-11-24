//! VGA 80x25 Text-Mode Color Engine & Framebuffer Blitter
//!
//! Manages the 80x25 character-attribute text buffer at 0xB8000 (mapped via
//! physical memory offset) and blits to the graphical framebuffer when active.
//! Implements CP437 box-drawing primitives, formatted console printing, and
//! full TUI window rendering.

use core::fmt;
use spin::Mutex;

pub const BUFFER_WIDTH: usize = 80;
pub const BUFFER_HEIGHT: usize = 25;
pub const VGA_PHYSICAL_ADDRESS: u64 = 0xB8000;

// CP437 Box-Drawing & Block Characters
pub const BOX_TOP_LEFT: u8 = 0xDA;          // ┌
pub const BOX_TOP_RIGHT: u8 = 0xBF;         // ┐
pub const BOX_BOTTOM_LEFT: u8 = 0xC0;      // └
pub const BOX_BOTTOM_RIGHT: u8 = 0xD9;     // ┘
pub const BOX_HORIZONTAL: u8 = 0xC4;       // ─
pub const BOX_VERTICAL: u8 = 0xB3;         // │

pub const BOX_DOUBLE_TOP_LEFT: u8 = 0xC9;     // ╔
pub const BOX_DOUBLE_TOP_RIGHT: u8 = 0xBB;    // ╗
pub const BOX_DOUBLE_BOTTOM_LEFT: u8 = 0xC8; // ╚
pub const BOX_DOUBLE_BOTTOM_RIGHT: u8 = 0xBC;// ╝
pub const BOX_DOUBLE_HORIZONTAL: u8 = 0xCD;  // ═
pub const BOX_DOUBLE_VERTICAL: u8 = 0xBA;    // ║

pub const BLOCK_FULL: u8 = 0xDB;   // █
pub const BLOCK_LIGHT: u8 = 0xB0;  // ░
pub const BLOCK_MEDIUM: u8 = 0xB1; // ▒
pub const BLOCK_DARK: u8 = 0xB2;   // ▓

/// Standard 16-color VGA palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

/// Computes a VGA color attribute byte: `(bg << 4) | (fg & 0x0F)`.
#[inline]
pub const fn color_attribute(fg: Color, bg: Color) -> u8 {
    ((bg as u8) << 4) | ((fg as u8) & 0x0F)
}

/// Standard 16-color VGA palette to 24-bit RGB mapping.
pub const VGA_PALETTE: [(u8, u8, u8); 16] = [
    (0x0C, 0x14, 0x24), // 0: Sleek Navy Dark Background
    (0x1B, 0x49, 0x94), // 1: Blue
    (0x2E, 0xC4, 0x7E), // 2: Green
    (0x00, 0xD4, 0xD4), // 3: Bright Cyan
    (0xE0, 0x31, 0x31), // 4: Red
    (0x9C, 0x36, 0xB5), // 5: Magenta
    (0xAA, 0x55, 0x00), // 6: Brown
    (0xCC, 0xD2, 0xE0), // 7: Light Gray
    (0x38, 0x42, 0x56), // 8: Dark Gray
    (0x55, 0x88, 0xFF), // 9: Light Blue
    (0x4E, 0xDC, 0x88), // 10: Light Green
    (0x38, 0xEF, 0xFF), // 11: Light Cyan
    (0xFF, 0x6B, 0x6B), // 12: Light Red
    (0xF7, 0x83, 0xAC), // 13: Pink
    (0xFF, 0xD4, 0x3B), // 14: Yellow
    (0xFF, 0xFF, 0xFF), // 15: White
];

/// A single cell in VGA text buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ScreenChar {
    pub ascii_character: u8,
    pub color_code: u8,
}

/// 80x25 character grid in VGA text memory.
#[repr(C)]
pub struct TextBuffer {
    pub chars: [[ScreenChar; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

/// Framebuffer metadata for graphical blitting.
pub struct FramebufferTarget {
    pub buffer_ptr: *mut u8,
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub bytes_per_pixel: usize,
    pub is_bgr: bool,
}

// FramebufferTarget raw pointer is safe to send between threads since access is protected by Mutex.
unsafe impl Send for FramebufferTarget {}

/// Global state maintaining VGA text memory pointer, shadow grid, and graphical fallback.
pub struct VgaDisplay {
    text_buffer_ptr: Option<*mut TextBuffer>,
    framebuffer: Option<FramebufferTarget>,
    shadow_grid: [[ScreenChar; BUFFER_WIDTH]; BUFFER_HEIGHT],
    pub column_position: usize,
    pub row_position: usize,
    pub current_color: u8,
}

// Safe to send across threads behind spin::Mutex.
unsafe impl Send for VgaDisplay {}

impl VgaDisplay {
    pub const fn new() -> Self {
        Self {
            text_buffer_ptr: None,
            framebuffer: None,
            shadow_grid: [[ScreenChar {
                ascii_character: b' ',
                color_code: 0x07,
            }; BUFFER_WIDTH]; BUFFER_HEIGHT],
            column_position: 0,
            row_position: 0,
            current_color: 0x07, // Light gray on black
        }
    }

    /// Sets the physical memory offset to map 0xB8000.
    pub fn set_text_buffer_offset(&mut self, phys_mem_offset: u64) {
        let virt_addr = phys_mem_offset + VGA_PHYSICAL_ADDRESS;
        self.text_buffer_ptr = Some(virt_addr as *mut TextBuffer);
    }

    /// Configures graphical framebuffer target.
    pub fn set_framebuffer(
        &mut self,
        buffer_ptr: *mut u8,
        width: usize,
        height: usize,
        stride: usize,
        bytes_per_pixel: usize,
        is_bgr: bool,
    ) {
        self.framebuffer = Some(FramebufferTarget {
            buffer_ptr,
            width,
            height,
            stride,
            bytes_per_pixel,
            is_bgr,
        });
    }

    /// Renders a single character cell to the hardware text buffer and graphical framebuffer.
    pub fn write_cell(&mut self, col: usize, row: usize, ascii_character: u8, color_code: u8) {
        if col >= BUFFER_WIDTH || row >= BUFFER_HEIGHT {
            return;
        }

        let cell = ScreenChar {
            ascii_character,
            color_code,
        };
        self.shadow_grid[row][col] = cell;

        // 1. Write to hardware VGA text buffer if mapped
        if let Some(buf_ptr) = self.text_buffer_ptr {
            unsafe {
                let cell_ptr = &mut (*buf_ptr).chars[row][col] as *mut ScreenChar;
                core::ptr::write_volatile(cell_ptr, cell);
            }
        }

        // 2. Blit to graphical framebuffer if present
        if let Some(ref fb) = self.framebuffer {
            Self::blit_cell_to_fb(fb, col, row, ascii_character, color_code);
        }
    }

    /// Blits an 8x8 character cell to the linear framebuffer using the font mapper.
    fn blit_cell_to_fb(
        fb: &FramebufferTarget,
        col: usize,
        row: usize,
        ascii_code: u8,
        color_code: u8,
    ) {
        let fg_idx = (color_code & 0x0F) as usize;
        let bg_idx = ((color_code >> 4) & 0x0F) as usize;
        let fg_rgb = VGA_PALETTE[fg_idx];
        let bg_rgb = VGA_PALETTE[bg_idx];

        let glyph = crate::font::get_glyph(ascii_code);

        // Calculate scaling based on framebuffer dimensions
        let cell_w = (fb.width / BUFFER_WIDTH).max(8);
        let cell_h = (fb.height / BUFFER_HEIGHT).max(8);

        let start_x = col * cell_w;
        let start_y = row * cell_h;

        for cy in 0..cell_h {
            let py = start_y + cy;
            if py >= fb.height {
                continue;
            }
            let glyph_y = (cy * 8) / cell_h;
            let row_byte = glyph[glyph_y.min(7)];

            for cx in 0..cell_w {
                let px = start_x + cx;
                if px >= fb.width {
                    continue;
                }
                let glyph_x = (cx * 8) / cell_w;
                let is_fg = (row_byte & (0x80 >> glyph_x.min(7))) != 0;
                let (r, g, b) = if is_fg { fg_rgb } else { bg_rgb };

                let pixel_offset = py * fb.stride * fb.bytes_per_pixel + px * fb.bytes_per_pixel;
                unsafe {
                    let p = fb.buffer_ptr.add(pixel_offset);
                    if fb.is_bgr {
                        *p = b;
                        *p.add(1) = g;
                        *p.add(2) = r;
                    } else {
                        *p = r;
                        *p.add(1) = g;
                        *p.add(2) = b;
                    }
                    if fb.bytes_per_pixel >= 4 {
                        *p.add(3) = 0xFF; // Full opacity
                    }
                }
            }
        }
    }

    /// Clears the entire 80x25 screen with spaces using the current color.
    pub fn clear(&mut self) {
        let blank = b' ';
        let color = self.current_color;
        for r in 0..BUFFER_HEIGHT {
            for c in 0..BUFFER_WIDTH {
                self.write_cell(c, r, blank, color);
            }
        }
        self.column_position = 0;
        self.row_position = 0;
    }

    /// Draws a CP437 single-line box.
    pub fn draw_box(&mut self, col: usize, row: usize, width: usize, height: usize, color: u8) {
        if width < 2 || height < 2 || col + width > BUFFER_WIDTH || row + height > BUFFER_HEIGHT {
            return;
        }

        let right = col + width - 1;
        let bottom = row + height - 1;

        // Corners
        self.write_cell(col, row, BOX_TOP_LEFT, color);
        self.write_cell(right, row, BOX_TOP_RIGHT, color);
        self.write_cell(col, bottom, BOX_BOTTOM_LEFT, color);
        self.write_cell(right, bottom, BOX_BOTTOM_RIGHT, color);

        // Top and bottom horizontal borders
        for c in (col + 1)..right {
            self.write_cell(c, row, BOX_HORIZONTAL, color);
            self.write_cell(c, bottom, BOX_HORIZONTAL, color);
        }

        // Left and right vertical borders and fill inner space
        for r in (row + 1)..bottom {
            self.write_cell(col, r, BOX_VERTICAL, color);
            self.write_cell(right, r, BOX_VERTICAL, color);
            for c in (col + 1)..right {
                self.write_cell(c, r, b' ', color);
            }
        }
    }

    /// Writes a text string starting at `(col, row)` with the given color.
    pub fn write_at(&mut self, mut col: usize, row: usize, text: &str, color: u8) {
        if row >= BUFFER_HEIGHT {
            return;
        }
        for byte in text.bytes() {
            if col >= BUFFER_WIDTH {
                break;
            }
            self.write_cell(col, row, byte, color);
            col += 1;
        }
    }

    /// Scrolls the screen content up by one row.
    pub fn scroll(&mut self) {
        for r in 1..BUFFER_HEIGHT {
            for c in 0..BUFFER_WIDTH {
                let cell = self.shadow_grid[r][c];
                self.write_cell(c, r - 1, cell.ascii_character, cell.color_code);
            }
        }
        let blank = b' ';
        let color = self.current_color;
        for c in 0..BUFFER_WIDTH {
            self.write_cell(c, BUFFER_HEIGHT - 1, blank, color);
        }
        if self.row_position > 0 {
            self.row_position -= 1;
        }
    }

    /// Writes a single byte handling control characters (`\n`, `\r`, `\x08`).
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => {
                self.column_position = 0;
                if self.row_position + 1 >= BUFFER_HEIGHT {
                    self.scroll();
                } else {
                    self.row_position += 1;
                }
            }
            b'\r' => {
                self.column_position = 0;
            }
            0x08 => {
                // Backspace
                if self.column_position > 0 {
                    self.column_position -= 1;
                    let color = self.current_color;
                    self.write_cell(self.column_position, self.row_position, b' ', color);
                }
            }
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.column_position = 0;
                    if self.row_position + 1 >= BUFFER_HEIGHT {
                        self.scroll();
                    } else {
                        self.row_position += 1;
                    }
                }
                let color = self.current_color;
                self.write_cell(self.column_position, self.row_position, byte, color);
                self.column_position += 1;
            }
        }
    }

    /// Writes a string slice to the console stream.
    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
    }
}

/// Implement `core::fmt::Write` for formatted output.
impl fmt::Write for VgaDisplay {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

/// Global synchronized VGA display engine.
pub static VGA: Mutex<VgaDisplay> = Mutex::new(VgaDisplay::new());

/// Initializes the VGA engine with physical memory offset and optional bootloader framebuffer.
pub fn init(
    phys_mem_offset: Option<u64>,
    framebuffer: Option<&'static mut bootloader_api::info::FrameBuffer>,
) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let mut vga = VGA.lock();
        if let Some(offset) = phys_mem_offset {
            vga.set_text_buffer_offset(offset);
        }
        if let Some(fb) = framebuffer {
            let info = fb.info();
            let is_bgr = match info.pixel_format {
                bootloader_api::info::PixelFormat::Bgr => true,
                _ => false,
            };
            let buf_ptr = fb.buffer_mut().as_mut_ptr();
            vga.set_framebuffer(
                buf_ptr,
                info.width,
                info.height,
                info.stride,
                info.bytes_per_pixel,
                is_bgr,
            );
        }
        vga.clear();
    });
}

/// Helper function to draw a box on the VGA display.
pub fn draw_box(col: usize, row: usize, width: usize, height: usize, color: u8) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        VGA.lock().draw_box(col, row, width, height, color);
    });
}

/// Helper function to write text at specific coordinates.
pub fn write_at(col: usize, row: usize, text: &str, color: u8) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        VGA.lock().write_at(col, row, text, color);
    });
}

/// Helper function to write a single character at specific coordinates.
pub fn write_char_at(col: usize, row: usize, ascii_character: u8, color: u8) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        VGA.lock().write_cell(col, row, ascii_character, color);
    });
}

/// Clears the VGA display.
pub fn clear() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        VGA.lock().clear();
    });
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        use core::fmt::Write;
        VGA.lock().write_fmt(args).unwrap();
    });
}

/// Prints formatted text to the VGA display without a newline.
#[macro_export]
macro_rules! vga_print {
    ($($arg:tt)*) => ($crate::vga::_print(format_args!($($arg)*)));
}

/// Prints formatted text to the VGA display with a trailing newline.
#[macro_export]
macro_rules! vga_println {
    () => ($crate::vga_print!("\n"));
    ($($arg:tt)*) => ($crate::vga_print!("{}\n", format_args!($($arg)*)));
}
