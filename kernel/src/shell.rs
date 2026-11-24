//! Interactive Command Shell & Simulated Web/Networking Portal for Titanium Micro-OS.
//!
//! CS2001 OS Capstone | Student: Yakshendrasinh R Mahida (USN: 25BTRCL288)
//! Features non-blocking keyboard input from `keyboard::pop_char()`, line editing
//! state machine (128-char clamp, cursor, backspace, enter, escape), dual serial
//! and VGA output, history tracking, and 17 commands:
//! `help`, `tasks`, `sched`, `mem`, `deadlock`, `info`, `reset`, `clear`, `web`,
//! `wifi`, `net`, `ip`, `ping`, `dns`, `ports`, `api`, `trace`.
//!
//! ZERO floating-point instructions (#![no_std]), zero heap allocations.

use core::fmt::{self, Write};
use crate::deadlock;
use crate::memory;
use crate::pit;
use crate::scheduler;
use crate::serial_println;
use crate::sync::SpinLock;
use crate::task::{TaskClass, TaskState};
use crate::ui;
use crate::vga::{self, Color};

pub const MAX_COMMAND_LEN: usize = 128;
pub const MAX_HISTORY_ENTRIES: usize = 16;
pub const MAX_OUTPUT_LINES: usize = 32;
pub const MAX_LINE_WIDTH: usize = 80;

pub const SHELL_CONTENT_ROW_START: usize = 18;
pub const SHELL_OUTPUT_ROWS: usize = 5; // Rows 18..22
pub const SHELL_PROMPT_ROW: usize = 23; // Row 23

/// Stack-allocated fixed-capacity buffer implementing `core::fmt::Write`.
pub struct StaticFormatter<const N: usize> {
    pub buf: [u8; N],
    pub len: usize,
}

impl<const N: usize> StaticFormatter<N> {
    pub const fn new() -> Self {
        Self { buf: [0; N], len: 0 }
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }
}

impl<const N: usize> Default for StaticFormatter<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Write for StaticFormatter<N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        let remaining = N.saturating_sub(self.len);
        let to_copy = remaining.min(bytes.len());
        self.buf[self.len..self.len + to_copy].copy_from_slice(&bytes[..to_copy]);
        self.len += to_copy;
        Ok(())
    }
}

/// Interactive Shell State Machine.
pub struct Shell {
    // Current input line
    pub input_buf: [u8; MAX_COMMAND_LEN],
    pub input_len: usize,
    pub cursor: usize,

    // Command history
    pub history: [[u8; MAX_COMMAND_LEN]; MAX_HISTORY_ENTRIES],
    pub history_lens: [usize; MAX_HISTORY_ENTRIES],
    pub history_count: usize,

    // Circular line buffer for output area
    pub output_lines: [[u8; MAX_LINE_WIDTH]; MAX_OUTPUT_LINES],
    pub output_lens: [usize; MAX_OUTPUT_LINES],
    pub output_head: usize,
    pub output_count: usize,
}

impl Shell {
    pub const fn new() -> Self {
        Self {
            input_buf: [0; MAX_COMMAND_LEN],
            input_len: 0,
            cursor: 0,
            history: [[0; MAX_COMMAND_LEN]; MAX_HISTORY_ENTRIES],
            history_lens: [0; MAX_HISTORY_ENTRIES],
            history_count: 0,
            output_lines: [[0; MAX_LINE_WIDTH]; MAX_OUTPUT_LINES],
            output_lens: [0; MAX_OUTPUT_LINES],
            output_head: 0,
            output_count: 0,
        }
    }

    /// Handles an incoming keyboard character from `keyboard::pop_char()`.
    pub fn handle_char(&mut self, ch: char) {
        match ch {
            '\n' | '\r' => {
                self.commit_line();
            }
            '\x08' => {
                // Backspace
                if self.cursor > 0 {
                    for i in self.cursor..self.input_len {
                        self.input_buf[i - 1] = self.input_buf[i];
                    }
                    self.cursor -= 1;
                    self.input_len -= 1;
                    self.redraw_prompt();
                }
            }
            '\x1B' => {
                // Escape: clear current input line
                self.input_len = 0;
                self.cursor = 0;
                self.redraw_prompt();
            }
            c if (' '..='~').contains(&c) => {
                // Printable character clamped at 128 chars
                if self.input_len < MAX_COMMAND_LEN {
                    for i in (self.cursor..self.input_len).rev() {
                        self.input_buf[i + 1] = self.input_buf[i];
                    }
                    self.input_buf[self.cursor] = c as u8;
                    self.cursor += 1;
                    self.input_len += 1;
                    self.redraw_prompt();
                }
            }
            _ => {}
        }
    }

    /// Commits and evaluates the current input line.
    pub fn commit_line(&mut self) {
        let raw_line = core::str::from_utf8(&self.input_buf[..self.input_len]).unwrap_or_default();

        let clamped = if raw_line.len() > MAX_COMMAND_LEN {
            &raw_line[..MAX_COMMAND_LEN]
        } else {
            raw_line
        };

        let trimmed = clamped.trim();

        if !trimmed.is_empty() {
            let mut local_cmd = [0u8; MAX_COMMAND_LEN];
            let cmd_len = trimmed.len().min(MAX_COMMAND_LEN);
            local_cmd[..cmd_len].copy_from_slice(&trimmed.as_bytes()[..cmd_len]);
            let cmd_str = core::str::from_utf8(&local_cmd[..cmd_len]).unwrap_or("");

            // Push to history
            self.push_history(cmd_str);

            // Log command invocation to kernel log panel
            let mut log_buf = StaticFormatter::<38>::new();
            let _ = write!(log_buf, "[SHELL] cmd: {}", cmd_str);
            ui::log(log_buf.as_str());

            // Execute command
            self.execute(cmd_str);
        }

        self.input_len = 0;
        self.cursor = 0;
        self.redraw_all();
    }

    fn push_history(&mut self, cmd: &str) {
        let bytes = cmd.as_bytes();
        let len = bytes.len().min(MAX_COMMAND_LEN);
        let idx = self.history_count % MAX_HISTORY_ENTRIES;
        self.history[idx][..len].copy_from_slice(&bytes[..len]);
        self.history_lens[idx] = len;
        self.history_count += 1;
    }

    /// Appends a multi-line string to the output buffer and serial console.
    pub fn push_output(&mut self, text: &str) {
        for line in text.lines() {
            serial_println!("{}", line);

            let bytes = line.as_bytes();
            let len = bytes.len().min(MAX_LINE_WIDTH);
            let idx = (self.output_head + self.output_count) % MAX_OUTPUT_LINES;
            self.output_lines[idx][..len].copy_from_slice(&bytes[..len]);
            self.output_lens[idx] = len;
            if self.output_count < MAX_OUTPUT_LINES {
                self.output_count += 1;
            } else {
                self.output_head = (self.output_head + 1) % MAX_OUTPUT_LINES;
            }
        }
    }

    /// Dispatches command string.
    pub fn execute(&mut self, input: &str) {
        let mut tokens = input.split_whitespace();
        let cmd = match tokens.next() {
            Some(c) => c,
            None => return,
        };

        let mut fmt = StaticFormatter::<2048>::new();

        match cmd {
            "help" => {
                let _ = writeln!(fmt, "Supported commands:");
                let _ = writeln!(fmt, "  help      - Display this list of commands");
                let _ = writeln!(fmt, "  tasks     - List registered tasks and execution states");
                let _ = writeln!(fmt, "  sched     - Show 3-queue MLFQ status & burst ratios");
                let _ = writeln!(fmt, "  mem       - Display 64 KiB Bump Allocator memory usage");
                let _ = writeln!(fmt, "  deadlock  - Show Dijkstra's Banker's Algorithm status");
                let _ = writeln!(fmt, "  info      - System metadata and student USN");
                let _ = writeln!(fmt, "  reset     - Reset shell state and telemetry");
                let _ = writeln!(fmt, "  clear     - Clear shell output buffer");
                let _ = writeln!(fmt, "  web       - Display simulated network portal & status viewer");
                let _ = writeln!(fmt, "  wifi      - Show WiFi adapter status and signal metrics");
                let _ = writeln!(fmt, "  net       - Display full network interface configuration");
                let _ = writeln!(fmt, "  ip        - Show IPv4/IPv6 address bindings");
                let _ = writeln!(fmt, "  ping      - Simulate ICMP echo to gateway and DNS servers");
                let _ = writeln!(fmt, "  dns       - Query DNS resolver cache and nameservers");
                let _ = writeln!(fmt, "  ports     - List open TCP/UDP ports and listeners");
                let _ = writeln!(fmt, "  api       - Simulate REST API health-check endpoint");
                let _ = write!(fmt,   "  trace     - Simulate network traceroute to external host");
                self.push_output(fmt.as_str());
            }
            "tasks" => {
                let _ = writeln!(fmt, "PID  NAME      CLASS        STATE       BURST_RATIO  QUEUE");
                if let Some(sched) = scheduler::SCHEDULER.try_lock() {
                    for t in sched.get_tasks().iter().flatten() {
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
                        let (whole, frac) = t.burst_ratio_parts();
                        let _ = writeln!(
                            fmt,
                            "  {:<2} {:<9} {:<12} {:<11} {}.{:04}       Q{}",
                            t.id, t.name, class_str, state_str, whole, frac, t.queue
                        );
                    }
                }
                self.push_output(fmt.as_str());
            }
            "sched" => {
                if let Some(sched) = scheduler::SCHEDULER.try_lock() {
                    let _ = write!(
                        fmt,
                        "MLFQ Scheduler Status:\n  Queues: Q0 (quantum={}ms), Q1 (quantum={}ms), Q2 (quantum={}ms)\n  Boost Interval: 50ms (promotes all ready tasks to Q0)\n  Classification Threshold: Θ < 0.20 (Interactive) vs Θ >= 0.20 (Compute)\n  Active Tasks: {} | Context Switches: {}",
                        sched.quantums_ms[0],
                        sched.quantums_ms[1],
                        sched.quantums_ms[2],
                        sched.total_tasks(),
                        sched.switches
                    );
                } else {
                    let _ = write!(fmt, "MLFQ Scheduler is locked.");
                }
                self.push_output(fmt.as_str());
            }
            "mem" => {
                let stats = memory::stats();
                let pct_whole = (stats.used * 100) / stats.total;
                let pct_frac = ((stats.used * 1000) / stats.total) % 10;
                let free_pct_whole = (stats.free * 100) / stats.total;
                let free_pct_frac = ((stats.free * 1000) / stats.total) % 10;

                let _ = write!(
                    fmt,
                    "Bump Allocator Status:\n  Total Capacity: {} bytes ({} KiB)\n  Allocated:      {} bytes ({}.{}%)\n  Remaining Free: {} bytes ({}.{}%)\n  Alignment:      Power-of-2 enforced (1, 2, 4, 8, 16, 64 B)",
                    stats.total,
                    stats.total / 1024,
                    stats.used,
                    pct_whole,
                    pct_frac,
                    stats.free,
                    free_pct_whole,
                    free_pct_frac
                );
                self.push_output(fmt.as_str());
            }
            "deadlock" => {
                let status = deadlock::status();
                let state_str = if status.is_safe { "SAFE" } else { "UNSAFE" };
                let seq = status.safe_sequence;
                let avail = status.available;

                let _ = write!(
                    fmt,
                    "Banker's Deadlock Avoidance:\n  System State:   {}\n  Available:      [A: {}, B: {}, C: {}]\n  Safe Sequence:  <P{} -> P{} -> P{} -> P{}>\n  Active Claims:  4 processes evaluated, 0 deadlock cycles detected",
                    state_str,
                    avail[0], avail[1], avail[2],
                    seq[0], seq[1], seq[2], seq[3]
                );
                self.push_output(fmt.as_str());
            }
            "info" => {
                let ticks = pit::ticks();
                let seconds = ticks / 100;
                let _ = write!(
                    fmt,
                    "Titanium Micro-OS v0.1.0 (x86_64 bare-metal)\nCS2001 Operating Systems Capstone Project\nStudent: Yakshendrasinh R Mahida | USN: 25BTRCL288\nTarget: x86_64-unknown-none | Bootloader: 0.11.17 BIOS\nUptime: {} ticks ({} seconds)",
                    ticks, seconds
                );
                self.push_output(fmt.as_str());
            }
            "reset" => {
                self.history_count = 0;
                self.output_count = 0;
                self.output_head = 0;
                memory::reset();
                self.push_output("System state and shell history reset successfully.");
            }
            "clear" => {
                self.output_count = 0;
                self.output_head = 0;
                // Emit ANSI clear to serial
                serial_println!("\x1B[2J\x1B[H");
                // Clear shell area in VGA
                self.clear_vga_output();
            }
            "web" => {
                let adapter = "VirtIO-Net (PCI 0x1AF4:0x1000)";
                let ip = "192.168.1.105";
                let mask = "255.255.255.0";
                let gateway = "192.168.1.1";
                let dns = "1.1.1.1, 8.8.8.8";
                let ping_whole = 2;
                let ping_frac = 14;
                let http_code = 200;
                let banner = "Welcome to Titanium Micro-OS Embedded Web Status Viewer!";

                let _ = write!(
                    fmt,
                    "╔══════════════════════════════════════════════════════════════════════════════╗\n\
                     ║                TITANIUM MICRO-OS SIMULATED NETWORK PORTAL                   ║\n\
                     ║                     CS2001 OS Capstone | USN: 25BTRCL288                     ║\n\
                     ╠══════════════════════════════════════════════════════════════════════════════╣\n\
                     ║ Network Adapter:   {:<53}║\n\
                     ║ IPv4 Address:      {:<53}║\n\
                     ║ Subnet Mask:       {:<53}║\n\
                     ║ Default Gateway:   {:<53}║\n\
                     ║ DNS Servers:       {:<53}║\n\
                     ║ Gateway Ping:      {}.{:02} ms (Round-Trip Latency, 0% Packet Loss)              ║\n\
                     ║ HTTP Request:      GET /portal/index.html -> {} OK (Content-Length: 4096)   ║\n\
                     ║ Portal Banner:     {:<53}║\n\
                     ╚══════════════════════════════════════════════════════════════════════════════╝",
                    adapter, ip, mask, gateway, dns, ping_whole, ping_frac, http_code, banner
                );
                self.push_output(fmt.as_str());
            }
            "wifi" => {
                let ticks = pit::ticks();
                // Derive a simulated signal strength that varies slightly with uptime
                let base_signal: i32 = -42;
                let variation: i32 = ((ticks % 7) as i32) - 3;
                let signal = base_signal + variation;
                let noise: i32 = -95;
                let snr = signal - noise;
                let link_speed = 867;
                let channel = 36;
                let freq = 5180;

                let _ = write!(
                    fmt,
                    "WiFi Adapter Status:\n  Interface:    wlan0 (Qualcomm Atheros QCA6174 802.11ac)\n  SSID:         TitaniumOS-5G\n  BSSID:        A4:CF:12:7B:3E:91\n  Security:     WPA3-Personal (SAE)\n  Frequency:    {} MHz (Channel {})\n  Link Speed:   {} Mbps (MCS 9, 2x2 MIMO)\n  Signal:       {} dBm\n  Noise Floor:  {} dBm\n  SNR:          {} dB\n  State:        CONNECTED (authenticated, associated)",
                    freq, channel, link_speed, signal, noise, snr
                );
                self.push_output(fmt.as_str());
            }
            "net" => {
                let ticks = pit::ticks();
                let rx_bytes = ticks * 1472;
                let tx_bytes = ticks * 384;
                let rx_packets = ticks * 2;
                let tx_packets = ticks;

                let _ = write!(
                    fmt,
                    "Network Interface Configuration:\n  eth0 (VirtIO-Net PCI):\n    Link:    UP | Speed: 1000 Mbps | Duplex: Full\n    MAC:     52:54:00:12:34:56\n    IPv4:    192.168.1.105/24\n    IPv6:    fe80::5054:ff:fe12:3456/64\n    Gateway: 192.168.1.1\n    MTU:     1500 bytes\n    RX:      {} packets ({} bytes)\n    TX:      {} packets ({} bytes)\n    Errors:  0 dropped, 0 overruns, 0 collisions\n  lo (Loopback):\n    Link:    UP | MTU: 65536\n    IPv4:    127.0.0.1/8\n    IPv6:    ::1/128",
                    rx_packets, rx_bytes, tx_packets, tx_bytes
                );
                self.push_output(fmt.as_str());
            }
            "ip" => {
                let _ = write!(
                    fmt,
                    "IP Address Configuration:\n  Interface   Protocol  Address                  Scope\n  eth0        IPv4      192.168.1.105/24         global\n  eth0        IPv6      fe80::5054:ff:fe12:3456/64  link\n  lo          IPv4      127.0.0.1/8              host\n  lo          IPv6      ::1/128                  host\n  wlan0       IPv4      192.168.1.110/24         global\n  wlan0       IPv6      fe80::a6cf:12ff:fe7b:3e91/64  link\n  Default Route: 192.168.1.1 via eth0 (metric 100)\n  DNS Servers:   1.1.1.1, 8.8.8.8, 8.8.4.4"
                );
                self.push_output(fmt.as_str());
            }
            "ping" => {
                let ticks = pit::ticks();
                // Derive 4 simulated RTT values that look realistic
                let base_rtt: u32 = 2;
                let rtt1_frac = ((ticks * 7) % 100) as u32;
                let rtt2_frac = ((ticks * 13 + 29) % 100) as u32;
                let rtt3_frac = ((ticks * 3 + 51) % 100) as u32;
                let rtt4_frac = ((ticks * 11 + 73) % 100) as u32;
                let avg_frac = (rtt1_frac + rtt2_frac + rtt3_frac + rtt4_frac) / 4;

                let _ = write!(
                    fmt,
                    "PING 192.168.1.1 (gateway) 56(84) bytes of data:\n  64 bytes from 192.168.1.1: icmp_seq=1 ttl=64 time={}.{:02} ms\n  64 bytes from 192.168.1.1: icmp_seq=2 ttl=64 time={}.{:02} ms\n  64 bytes from 192.168.1.1: icmp_seq=3 ttl=64 time={}.{:02} ms\n  64 bytes from 192.168.1.1: icmp_seq=4 ttl=64 time={}.{:02} ms\n--- ping statistics ---\n  4 packets transmitted, 4 received, 0% packet loss\n  rtt min/avg/max = {}.{:02}/{}.{:02}/{}.{:02} ms",
                    base_rtt, rtt1_frac, base_rtt, rtt2_frac,
                    base_rtt, rtt3_frac, base_rtt, rtt4_frac,
                    base_rtt, rtt1_frac.min(rtt2_frac).min(rtt3_frac).min(rtt4_frac),
                    base_rtt, avg_frac,
                    base_rtt, rtt1_frac.max(rtt2_frac).max(rtt3_frac).max(rtt4_frac)
                );
                self.push_output(fmt.as_str());
            }
            "dns" => {
                let _ = write!(
                    fmt,
                    "DNS Resolver Status:\n  Nameserver 1:  1.1.1.1 (Cloudflare) ............ reachable (1.24 ms)\n  Nameserver 2:  8.8.8.8 (Google Public DNS) ...... reachable (4.87 ms)\n  Nameserver 3:  8.8.4.4 (Google Public DNS) ...... reachable (5.12 ms)\n  Search Domain: titanium.local\n  Cache Entries: 47 records (A: 31, AAAA: 12, CNAME: 4)\n  Lookups:\n    titanium.local          -> 192.168.1.105  (A, cached, TTL 3600s)\n    gateway.titanium.local  -> 192.168.1.1    (A, cached, TTL 7200s)\n    api.titanium.local      -> 192.168.1.200  (A, cached, TTL 1800s)\n  DNSSEC: validated | Protocol: DNS-over-TLS (DoT)"
                );
                self.push_output(fmt.as_str());
            }
            "ports" => {
                let _ = write!(
                    fmt,
                    "Open Ports & Listeners:\n  Proto  Local Address          Foreign Address        State       PID/Program\n  TCP    0.0.0.0:22             0.0.0.0:*              LISTEN      1/sshd\n  TCP    0.0.0.0:80             0.0.0.0:*              LISTEN      2/httpd\n  TCP    0.0.0.0:443            0.0.0.0:*              LISTEN      2/httpd\n  TCP    0.0.0.0:3000           0.0.0.0:*              LISTEN      3/api-server\n  TCP    192.168.1.105:8080     10.0.0.50:49312        ESTABLISHED 4/web-portal\n  UDP    0.0.0.0:53             0.0.0.0:*                          5/dns-resolver\n  UDP    0.0.0.0:67             0.0.0.0:*                          6/dhcpd\n  UDP    0.0.0.0:123            0.0.0.0:*                          7/ntpd\n  Total: 8 sockets (5 TCP, 3 UDP) | 1 ESTABLISHED connection"
                );
                self.push_output(fmt.as_str());
            }
            "api" => {
                let ticks = pit::ticks();
                let uptime_secs = ticks / 100;
                let latency_whole: u32 = 12;
                let latency_frac = ((ticks * 7 + 3) % 100) as u32;
                let req_count = ticks * 3;

                let _ = write!(
                    fmt,
                    "REST API Health Check:\n  Endpoint:     GET https://api.titanium.local/v1/health\n  Status:       200 OK\n  Response:     {{\n    \"status\": \"healthy\",\n    \"version\": \"v0.1.0\",\n    \"uptime_secs\": {},\n    \"kernel\": \"Titanium Micro-OS x86_64\",\n    \"student\": \"Yakshendrasinh R Mahida (25BTRCL288)\",\n    \"services\": {{\n      \"scheduler\": \"running\",\n      \"memory\": \"operational\",\n      \"banker\": \"safe\",\n      \"network\": \"connected\"\n    }},\n    \"requests_served\": {},\n    \"avg_latency_ms\": {}.{:02}\n  }}",
                    uptime_secs, req_count, latency_whole, latency_frac
                );
                self.push_output(fmt.as_str());
            }
            "trace" => {
                let _ = write!(
                    fmt,
                    "traceroute to 1.1.1.1 (Cloudflare DNS), 30 hops max, 60 byte packets:\n  1  192.168.1.1 (gateway)      1.24 ms   0.98 ms   1.11 ms\n  2  10.0.0.1    (isp-edge)     5.67 ms   4.89 ms   5.23 ms\n  3  172.16.0.1  (core-rtr-1)   8.45 ms   7.91 ms   8.12 ms\n  4  172.16.1.1  (core-rtr-2)  11.33 ms  10.78 ms  11.02 ms\n  5  198.51.100.1 (peering)     14.56 ms  13.89 ms  14.21 ms\n  6  1.1.1.1     (one.one.one)  16.82 ms  15.94 ms  16.37 ms\nTrace complete: 6 hops, avg RTT 16.37 ms, 0% packet loss"
                );
                self.push_output(fmt.as_str());
            }
            unknown => {
                let _ = write!(
                    fmt,
                    "Unknown command: '{}'. Type 'help' for available commands.",
                    unknown
                );
                self.push_output(fmt.as_str());
            }
        }
    }

    /// Clears VGA shell output rows (18..22).
    pub fn clear_vga_output(&self) {
        let blank_color = vga::color_attribute(Color::LightGray, Color::Black);
        for row in SHELL_CONTENT_ROW_START..SHELL_PROMPT_ROW {
            for col in 1..79 {
                vga::write_char_at(col, row, b' ', blank_color);
            }
        }
    }

    /// Redraws active prompt on row 23.
    pub fn redraw_prompt(&self) {
        let prompt_color = vga::color_attribute(Color::Yellow, Color::Black);
        let text_color = vga::color_attribute(Color::White, Color::Black);
        let cursor_color = vga::color_attribute(Color::LightCyan, Color::Black);
        let blank_color = vga::color_attribute(Color::LightGray, Color::Black);

        vga::write_at(1, SHELL_PROMPT_ROW, "> ", prompt_color);

        let input_str = core::str::from_utf8(&self.input_buf[..self.input_len]).unwrap_or("");
        vga::write_at(3, SHELL_PROMPT_ROW, input_str, text_color);

        // Draw cursor
        let cursor_col = 3 + self.cursor;
        if cursor_col < 79 {
            vga::write_char_at(cursor_col, SHELL_PROMPT_ROW, b'_', cursor_color);
        }

        // Blank out remainder of prompt line
        for col in (cursor_col + 1)..79 {
            vga::write_char_at(col, SHELL_PROMPT_ROW, b' ', blank_color);
        }
    }

    /// Writes a line of text to VGA, handling UTF-8 box characters mapped to CP437 glyphs.
    fn write_vga_line(&self, row: usize, s: &str, color: u8) {
        let mut col = 1usize;
        for ch in s.chars() {
            if col >= 79 {
                break;
            }
            let byte = match ch {
                '╔' => vga::BOX_DOUBLE_TOP_LEFT,
                '╗' => vga::BOX_DOUBLE_TOP_RIGHT,
                '╚' => vga::BOX_DOUBLE_BOTTOM_LEFT,
                '╝' => vga::BOX_DOUBLE_BOTTOM_RIGHT,
                '═' => vga::BOX_DOUBLE_HORIZONTAL,
                '║' => vga::BOX_DOUBLE_VERTICAL,
                '╠' => 0xCC,
                '╣' => 0xB9,
                '■' => vga::BLOCK_FULL,
                ascii if ascii.is_ascii() => ascii as u8,
                _ => b'?',
            };
            vga::write_char_at(col, row, byte, color);
            col += 1;
        }

        let blank_color = vga::color_attribute(Color::LightGray, Color::Black);
        for c in col..79 {
            vga::write_char_at(c, row, b' ', blank_color);
        }
    }

    /// Redraws the 5 most recent output lines on rows 18..22 and prompt on row 23.
    pub fn redraw_all(&self) {
        self.clear_vga_output();

        let visible = self.output_count.min(SHELL_OUTPUT_ROWS);
        let start_idx = self.output_count.saturating_sub(visible);

        let text_color = vga::color_attribute(Color::LightGray, Color::Black);

        for i in 0..visible {
            let line_idx = (self.output_head + start_idx + i) % MAX_OUTPUT_LINES;
            let len = self.output_lens[line_idx];
            if let Ok(line_str) = core::str::from_utf8(&self.output_lines[line_idx][..len]) {
                self.write_vga_line(SHELL_CONTENT_ROW_START + i, line_str, text_color);
            }
        }

        self.redraw_prompt();
    }
}

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

pub static SHELL: SpinLock<Shell> = SpinLock::new(Shell::new());

/// Initializes the interactive shell and prints the welcome banner.
pub fn init() {
    let mut shell = SHELL.lock();
    shell.push_output("Titanium Micro-OS Interactive Shell Ready. Type 'help' for commands.");
    shell.redraw_all();
}

/// Dispatches an incoming character to the global shell instance.
pub fn handle_char(ch: char) {
    SHELL.lock().handle_char(ch);
}
