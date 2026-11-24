//! PS/2 Keyboard Driver & Scancode Set 1 Ring Buffer
//!
//! Receives raw scancodes from IRQ1 (port 0x60), decodes Set 1 make/break
//! codes into ASCII characters (handling Shift, CapsLock, Enter, Backspace,
//! Space, punctuation, and alphanumeric keys), and enqueues them into a
//! non-blocking ring buffer.

use spin::Mutex;

const BUFFER_CAPACITY: usize = 128;

/// Scancode Set 1 unshifted ASCII mapping for make codes 0x01..0x39.
static SCANCODE_MAP_NORMAL: [char; 0x3A] = [
    '\0',   // 0x00: Invalid
    '\x1B', // 0x01: Escape
    '1',    // 0x02
    '2',    // 0x03
    '3',    // 0x04
    '4',    // 0x05
    '5',    // 0x06
    '6',    // 0x07
    '7',    // 0x08
    '8',    // 0x09
    '9',    // 0x0A
    '0',    // 0x0B
    '-',    // 0x0C
    '=',    // 0x0D
    '\x08', // 0x0E: Backspace
    '\t',   // 0x0F: Tab
    'q',    // 0x10
    'w',    // 0x11
    'e',    // 0x12
    'r',    // 0x13
    't',    // 0x14
    'y',    // 0x15
    'u',    // 0x16
    'i',    // 0x17
    'o',    // 0x18
    'p',    // 0x19
    '[',    // 0x1A
    ']',    // 0x1B
    '\n',   // 0x1C: Enter
    '\0',   // 0x1D: Left Control
    'a',    // 0x1E
    's',    // 0x1F
    'd',    // 0x20
    'f',    // 0x21
    'g',    // 0x22
    'h',    // 0x23
    'j',    // 0x24
    'k',    // 0x25
    'l',    // 0x26
    ';',    // 0x27
    '\'',   // 0x28
    '`',    // 0x29
    '\0',   // 0x2A: Left Shift (make)
    '\\',   // 0x2B
    'z',    // 0x2C
    'x',    // 0x2D
    'c',    // 0x2E
    'v',    // 0x2F
    'b',    // 0x30
    'n',    // 0x31
    'm',    // 0x32
    ',',    // 0x33
    '.',    // 0x34
    '/',    // 0x35
    '\0',   // 0x36: Right Shift (make)
    '*',    // 0x37: Keypad *
    '\0',   // 0x38: Left Alt
    ' ',    // 0x39: Space
];

/// Scancode Set 1 shifted ASCII mapping for make codes 0x01..0x39.
static SCANCODE_MAP_SHIFTED: [char; 0x3A] = [
    '\0',   // 0x00
    '\x1B', // 0x01
    '!',    // 0x02
    '@',    // 0x03
    '#',    // 0x04
    '$',    // 0x05
    '%',    // 0x06
    '^',    // 0x07
    '&',    // 0x08
    '*',    // 0x09
    '(',    // 0x0A
    ')',    // 0x0B
    '_',    // 0x0C
    '+',    // 0x0D
    '\x08', // 0x0E: Backspace
    '\t',   // 0x0F: Tab
    'Q',    // 0x10
    'W',    // 0x11
    'E',    // 0x12
    'R',    // 0x13
    'T',    // 0x14
    'Y',    // 0x15
    'U',    // 0x16
    'I',    // 0x17
    'O',    // 0x18
    'P',    // 0x19
    '{',    // 0x1A
    '}',    // 0x1B
    '\n',   // 0x1C: Enter
    '\0',   // 0x1D: Left Control
    'A',    // 0x1E
    'S',    // 0x1F
    'D',    // 0x20
    'F',    // 0x21
    'G',    // 0x22
    'H',    // 0x23
    'J',    // 0x24
    'K',    // 0x25
    'L',    // 0x26
    ':',    // 0x27
    '"',    // 0x28
    '~',    // 0x29
    '\0',   // 0x2A: Left Shift
    '|',    // 0x2B
    'Z',    // 0x2C
    'X',    // 0x2D
    'C',    // 0x2E
    'V',    // 0x2F
    'B',    // 0x30
    'N',    // 0x31
    'M',    // 0x32
    '<',    // 0x33
    '>',    // 0x34
    '?',    // 0x35
    '\0',   // 0x36: Right Shift
    '*',    // 0x37
    '\0',   // 0x38: Left Alt
    ' ',    // 0x39: Space
];

/// Non-blocking keyboard ring buffer and modifier state.
struct KeyboardState {
    buffer: [char; BUFFER_CAPACITY],
    head: usize,
    tail: usize,
    shift: bool,
    caps_lock: bool,
}

impl KeyboardState {
    const fn new() -> Self {
        Self {
            buffer: ['\0'; BUFFER_CAPACITY],
            head: 0,
            tail: 0,
            shift: false,
            caps_lock: false,
        }
    }

    fn push(&mut self, c: char) {
        let next_head = (self.head + 1) % BUFFER_CAPACITY;
        if next_head != self.tail {
            self.buffer[self.head] = c;
            self.head = next_head;
        }
    }

    fn pop(&mut self) -> Option<char> {
        if self.head == self.tail {
            None
        } else {
            let c = self.buffer[self.tail];
            self.tail = (self.tail + 1) % BUFFER_CAPACITY;
            Some(c)
        }
    }

    fn handle_scancode(&mut self, scancode: u8) {
        match scancode {
            // Shift make codes
            0x2A | 0x36 => self.shift = true,
            // Shift break codes
            0xAA | 0xB6 => self.shift = false,
            // CapsLock toggle make code
            0x3A => self.caps_lock = !self.caps_lock,
            // Make code (key press)
            code if code < 0x80 => {
                let idx = code as usize;
                if idx < SCANCODE_MAP_NORMAL.len() {
                    let mut ch = if self.shift {
                        SCANCODE_MAP_SHIFTED[idx]
                    } else {
                        SCANCODE_MAP_NORMAL[idx]
                    };

                    // Handle CapsLock letter case inversion
                    if self.caps_lock {
                        if ch.is_ascii_lowercase() {
                            ch = ch.to_ascii_uppercase();
                        } else if ch.is_ascii_uppercase() {
                            ch = ch.to_ascii_lowercase();
                        }
                    }

                    if ch != '\0' {
                        self.push(ch);
                    }
                }
            }
            // Break codes (key releases)
            _ => {}
        }
    }
}

static KEYBOARD: Mutex<KeyboardState> = Mutex::new(KeyboardState::new());

/// Processes a raw PS/2 scancode from port 0x60. Called from IRQ1 interrupt handler.
pub fn handle_scancode(scancode: u8) {
    KEYBOARD.lock().handle_scancode(scancode);
}

/// Non-blocking retrieval of the next decoded character from the keyboard ring buffer.
pub fn pop_char() -> Option<char> {
    x86_64::instructions::interrupts::without_interrupts(|| KEYBOARD.lock().pop())
}
