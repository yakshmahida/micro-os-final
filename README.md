# Titanium Micro-OS: Next-Gen Educational x86_64 Rust Microkernel

[![Rust](https://img.shields.io/badge/Rust-nightly-orange?logo=rust)](https://www.rust-lang.org/)
[![Target](https://img.shields.io/badge/target-x86__64--unknown--none-111827)](https://doc.rust-lang.org/)
[![Hypervisor](https://img.shields.io/badge/hypervisor-VirtualBox%207.2-183a61)](https://www.virtualbox.org/)
[![Host](https://img.shields.io/badge/host-Apple%20MacBook%20Air%20M4-black?logo=apple)](https://www.apple.com/)
[![Tests](https://img.shields.io/badge/tests-111%20passing-2f855a)](tests/)

> **Build the mechanism. Boot the kernel. Read the evidence.**

Titanium Micro-OS is a freestanding, bare-metal x86_64 microkernel engineered completely from scratch in safe, idiomatic Rust (`no_std`, `no_main`) for the **Operating Systems Continuous Assessment 6 (CA-6)** capstone challenge (CS2001 / Sem-3).

---

## Academic & Candidate Profile

| Academic Parameter | Submission Detail |
|---|---|
| **Candidate Name** | **Yakshendrasinh R Mahida** |
| **University Serial Number (USN)** | **25BTRCL288** |
| **Branch & Section** | **B.Tech Computer Science & Engineering — AIML** |
| **Semester & Curriculum** | **Semester 3 (3AIML) — Operating Systems (CS2001 / 25PCCS201)** |
| **Host Machine Architecture** | **Apple MacBook Air M4** (Apple Silicon) |
| **Virtualization Environment** | **Oracle VirtualBox 7.2** with `EnableX86OnArm` Binary Translation |
| **Research Specialization** | **Track R4: Workload-Aware MLFQ Scheduling & Banker's Deadlock Avoidance** |

---

## Key System Architecture

```text
+--------------------------------------------------------------------------+
|                  Interactive Shell & TUI Dashboard                       |
|   (help · tasks · sched · mem · deadlock · info · reset · clear · web)   |
+--------------------------------------------------------------------------+
|          Research Track R4 Workload-Aware MLFQ CPU Scheduler            |
|       (Q0: 4ms, Q1: 8ms, Q2: 32ms FCFS | 50ms Priority Boost | Theta)     |
+--------------------------------------------------------------------------+
|          Deadlock Avoidance: Dijkstra's Banker's Algorithm              |
+--------------------------------------------------------------------------+
|          Memory Subsystem: 64 KiB Monotonic Bump Allocator               |
+--------------------------------------------------------------------------+
|          Synchronization: Atomic SpinLock<T> (x86 PAUSE Hint)           |
+--------------------------------------------------------------------------+
|          Interrupts & Drivers (IDT 64-bit, PIC 8259, PIT 8253 100Hz)    |
|          VGA 80x25 TUI Engine · PS/2 Keyboard IRQ1 · UART 16550 Serial  |
+--------------------------------------------------------------------------+
|          Bootloader 0.11 & El Torito Hybrid ISO / VDI Pipeline           |
+--------------------------------------------------------------------------+
```

---

## Quick Start: Build & Launch in VirtualBox

### 1. Build Kernel & Generate Bootable Images
```bash
cargo build
```
This automatically produces:
- `dist/micro-os.iso` — Bootable El Torito ISO (3.0 MB)
- `dist/micro-os-bios.vdi` — VirtualBox Dynamic VDI Hard Disk (4.0 MB)
- `dist/micro-os-bios.img` — Raw BIOS disk image (3.5 MB)

### 2. Autonomous VirtualBox Launch (One Command)
```bash
bash scripts/run_virtualbox.sh
```
This automatically:
1. Shuts down and cleans any previous VM instance
2. Creates and registers the VM `TitaniumMicroOS` (x86 architecture, BIOS firmware, 128 MB RAM)
3. Configures Apple Silicon binary translation (`EnableX86OnArm 1`)
4. Attaches `dist/micro-os.iso` and `dist/micro-os-bios.vdi` to the IDE controller
5. Connects UART 16550 serial telemetry to `traces/qemu-boot.log`
6. Launches the VirtualBox GUI window live on your screen

### 3. Run Automated Host Test Suites
```bash
# Core mechanisms unit tests (9 passed)
cargo test --test kernel_mechanisms

# End-to-end multi-tier test harness (102 passed)
bash tests/run_e2e.sh
```

### 4. Analyze Serial Telemetry
```bash
python3 scripts/analyze_trace.py traces/qemu-boot.log
```

---

## Interactive Shell Commands

Inside the live VirtualBox VM window, use your keyboard to interact with the shell:

| Command | Action |
|---|---|
| `help` | Displays index of all supported kernel shell commands. |
| `tasks` | Lists active tasks, states, remaining execution steps, and live burst ratios $\Theta$. |
| `sched` | Displays 3-queue MLFQ status, quantum allocation, and priority boost metrics. |
| `mem` | Displays 64 KiB bump allocator statistics and visual utilization meter. |
| `deadlock` | Runs Banker's Algorithm safety check and prints the safe execution sequence. |
| `info` | Displays hardware topology (MacBook Air M4 host, VirtualBox 7.2 x86 VM, RAM, CPU). |
| `web` | **Simulated Network Portal**: displays VirtIO-Net adapter details, IP (`192.168.1.105`), DNS resolution, ping latency, HTTP 200 OK status, and student capstone portal header! |
| `reset` | Re-initializes all tasks and resets the MLFQ scheduler. |
| `clear` | Clears the shell terminal view. |

---

## Research Track R4: Workload-Aware MLFQ Scheduling

Empirical burst ratio $\Theta_i$ formulation:
$$\Theta_i = \frac{t_{\text{exec}}}{t_{\text{exec}} + t_{\text{wait}} + t_{\text{yield}}}$$

- **Interactive Tasks ($\Theta < 0.20$, Class $C_1$)**: Assigned to $Q_0$ ($q = 4\text{ ms}$) for sub-millisecond dispatching.
- **Compute-Bound Tasks ($\Theta \ge 0.20$, Class $C_2$)**: Demoted to $Q_2$ (FCFS / $q = 32\text{ ms}$), preventing context-switch thrashing.
- **Starvation Freedom**: Global periodic priority boost executes every $50\text{ ms}$ resetting aging counters.
