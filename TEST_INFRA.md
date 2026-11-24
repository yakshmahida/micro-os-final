# E2E Test Infra: Titanium Micro-OS

## Test Philosophy
- **Opaque-Box Verification**: Evaluates Titanium Micro-OS strictly through external verification surfaces without hooking into private kernel internals or compromising `no_std` kernel builds.
- **Requirement-Driven**: All test cases derive directly from user requirements R1 through R6 in `ORIGINAL_REQUEST.md` and feature allocations in `PROJECT.md`.
- **4-Tier Methodology**:
  - **Tier 1 — Feature Coverage (Happy Path)**: $\ge 5$ distinct happy-path tests per feature area across all 8 feature areas ($\ge 40$ tests).
  - **Tier 2 — Boundary & Corner Cases (Stress & Limits)**: $\ge 5$ boundary/corner tests per feature area where limits exist ($\ge 40$ tests).
  - **Tier 3 — Cross-Feature Combinations (Pairwise Interop)**: $\ge 10$ pairwise and multi-subsystem interaction test cases.
  - **Tier 4 — Real-World Application Scenarios (Workloads)**: $\ge 5$ realistic end-user multi-step workload scenarios.
- **Total Test Count**: 102 comprehensive test cases (42 Tier 1, 42 Tier 2, 12 Tier 3, 6 Tier 4).

## Verification Surfaces
1. **Surface A — File System & Artifact Inspection**: Verifies toolchain configuration (`rust-toolchain.toml`, `Cargo.toml`, `.cargo/config.toml`, `build.rs`) and binary deliverables (`dist/micro-os.iso` ISO 9660 PVD Sector 16, El Torito Boot Record Sector 17, Boot Catalog Sector 19, `dist/micro-os-bios.vdi` header).
2. **Surface B — Public Host Library Interface (`src/lib.rs`)**: Verifies core OS mechanisms via public APIs (`Task`, `MlfqScheduler`, `BumpAllocator`, `BankersAlgorithm`, `SpinLock`) compiled with `std` on the host.
3. **Surface C — Command & TUI Contract Evaluation**: Verifies shell command dispatch, argument parsing, error recovery for all 9 commands (`help`, `tasks`, `sched`, `mem`, `deadlock`, `info`, `reset`, `clear`, `web`), and asserts simulated network metrics for the `web` portal viewer.
4. **Surface D — Driver & Hardware Register Encoding**: Validates mathematical and encoding contracts (VGA color attributes `(bg << 4) | fg`, CP437 box-drawing mappings, UART baud divisor, PIC remap offsets 32..47, PIT 100 Hz divisor 11932, PS/2 scancode decoding).
5. **Surface E — VirtualBox Automation & Serial Trace Pipeline**: Validates `scripts/run_virtualbox.sh` CLI configurations, `traces/qemu-boot.log` regex conformance (`\[SCHED\] tick=(\d+) task=(\d+) \((\w+)\) Q=(\d) .* Θ=([\d.]+)`), and `scripts/analyze_trace.py` execution metrics.

## Test Architecture & Layout
```
/Users/yakshmahida/Desktop/rusto-os-original-yaksh/
├── tests/
│   ├── kernel_mechanisms.rs          # Host unit test suite (>= 7 tests, R6)
│   ├── run_e2e.sh                    # Standalone shell runner script
│   ├── e2e_suite.rs                  # Master E2E integration test entry
│   └── e2e/                          # Modular E2E test modules
│       ├── mod.rs                    # Re-exports & module declarations
│       ├── common.rs                 # Opaque harness utilities & binary parsers
│       ├── build_pipeline.rs         # R1: Build & ISO Pipeline tests (10 tests)
│       ├── drivers_and_interrupts.rs # R2: Hardware driver contracts (10 tests)
│       ├── mechanisms_mlfq.rs        # R3: MLFQ & Task tests (10 tests)
│       ├── memory_and_sync.rs        # R3: Bump Allocator & SpinLock (10 tests)
│       ├── deadlock_banker.rs        # R3: Banker's Deadlock Avoidance (10 tests)
│       ├── tui_and_shell.rs          # R4: TUI layout & 9 Shell commands (10 tests)
│       ├── vbox_and_trace.rs         # R5 & R6: VirtualBox & Trace script (10 tests)
│       └── scenarios.rs              # T3: Cross-Feature (12) + T4: Scenarios (6)
```

## Test Runner & Execution Semantics
- **Master Test Command**:
  ```bash
  cargo test --test e2e_suite
  ```
- **Tier-Specific Filtering**:
  ```bash
  cargo test --test e2e_suite -- t1_     # Run Tier 1 Feature Coverage tests
  cargo test --test e2e_suite -- t2_     # Run Tier 2 Boundary & Corner tests
  cargo test --test e2e_suite -- t3_     # Run Tier 3 Cross-Feature tests
  cargo test --test e2e_suite -- t4_     # Run Tier 4 Real-World Application Scenarios
  ```
- **Module-Specific Filtering**:
  ```bash
  cargo test --test e2e_suite -- build_pipeline
  cargo test --test e2e_suite -- drivers_and_interrupts
  cargo test --test e2e_suite -- mechanisms_mlfq
  cargo test --test e2e_suite -- memory_and_sync
  cargo test --test e2e_suite -- deadlock_banker
  cargo test --test e2e_suite -- tui_and_shell
  cargo test --test e2e_suite -- vbox_and_trace
  ```
- **Standalone Runner Script**:
  ```bash
  bash tests/run_e2e.sh
  ```
  - Exit code `0`: All tests passed.
  - Exit code `1`: Assertion failure in test case.
  - Exit code `2`: Compilation / build error.
  - Exit code `3`: Missing environment prerequisites.

---

## Complete 4-Tier Test Case Inventory (102 Test Cases)

### Tier 1: Feature Coverage (42 Test Cases)
| Test ID | Area | Test Name | Objective | Verification Surface |
|---------|------|-----------|-----------|----------------------|
| `TC-T1-01` | R1 | Toolchain Config Validity | Nightly channel, `x86_64-unknown-none`, `rust-src`, `llvm-tools-preview` | Surface A |
| `TC-T1-02` | R1 | Cargo Bindeps Config | `.cargo/config.toml` contains `[unstable] bindeps = true` | Surface A |
| `TC-T1-03` | R1 | Workspace Manifest Structure | Root `Cargo.toml` declares workspace members `["kernel"]` | Surface A |
| `TC-T1-04` | R1 | ISO 9660 & El Torito Generation | Validates `dist/micro-os.iso`, Sector 16 PVD "CD001", Boot Catalog | Surface A |
| `TC-T1-05` | R1 | VirtualBox VDI Disk Header | Validates `dist/micro-os-bios.vdi` signature `<<< Oracle VM VirtualBox Disk Image >>>` | Surface A |
| `TC-T1-06` | R2 | VGA Memory Layout & Color Encoding | Validates character and attribute byte encoding `(bg << 4) | fg` | Surface D |
| `TC-T1-07` | R2 | CP437 Box-Drawing Mappings | Validates Unicode box glyphs map to CP437 bytes (`0xC4`, `0xB3`, `0xDA`, etc.) | Surface D |
| `TC-T1-08` | R2 | UART 16550 Serial Initialization | Validates COM1 `0x3F8` 115200 baud divisor = 1, 8N1 line control | Surface D |
| `TC-T1-09` | R2 | 64-Bit IDT Descriptor Setup | Validates IDT gates for vectors 8, 13, 14, 32, 33 with type `0x8E` | Surface D |
| `TC-T1-10` | R2 | PIC 8259 Cascade Remapping | Validates PIC remap offsets (Master: 32, Slave: 40) and EOI protocol | Surface D |
| `TC-T1-11` | R2 | PS/2 Keyboard Scancode Decoding | Validates Make scancodes translate to ASCII in ring buffer | Surface D |
| `TC-T1-12` | R3 | Task Control Block Lifecycle | Validates task states `Ready` -> `Running` -> `Terminated` | Surface B |
| `TC-T1-13` | R3 | 3-Queue MLFQ Quantum Demotion | Validates compute tasks demote Q0 (4ms) -> Q1 (8ms) -> Q2 (32ms) | Surface B |
| `TC-T1-14` | R3 | Burst Ratio $\Theta$ Calculation | Validates $\Theta = \frac{t_{\text{exec}}}{t_{\text{exec}} + t_{\text{wait}} + t_{\text{yield}}}$ precision | Surface B |
| `TC-T1-15` | R3 | Periodic 50ms Priority Boost | Validates all tasks in Q1/Q2 promoted to Q0 every 5 ticks (50ms) | Surface B |
| `TC-T1-16` | R3 | Deterministic Scheduler Replay | Validates identical task parameters produce byte-identical schedules | Surface B |
| `TC-T1-17` | R3 | 64 KiB Heap Allocation & Tracking | Validates allocation updates `used` and `free` counters accurately | Surface B |
| `TC-T1-18` | R3 | Power-of-2 Alignment Enforcement | Validates addresses satisfy 1, 2, 4, 8, 16, 64-byte alignment | Surface B |
| `TC-T1-19` | R3 | Contiguous Monotonic Allocations | Validates sequential allocations increase monotonically without overlap | Surface B |
| `TC-T1-20` | R3 | SpinLock Mutual Exclusion | Validates atomic pause spin loop serializes concurrent access | Surface B |
| `TC-T1-21` | R3 | SpinLock RAII Guard Drop Safety | Validates lock automatically releases when guard drops | Surface B |
| `TC-T1-22` | R3 | Banker Safe State Detection | Validates known safe state evaluates to `is_safe() == true` | Surface B |
| `TC-T1-23` | R3 | Banker Safe Sequence Generation | Validates `safe_sequence()` derives valid topological execution order | Surface B |
| `TC-T1-24` | R3 | Banker Safe Request Approval | Validates safe resource request is granted and matrices update | Surface B |
| `TC-T1-25` | R3 | Banker Unsafe Request Rejection | Validates deadlock-inducing request is rejected and state rolled back | Surface B |
| `TC-T1-26` | R3 | Banker Resource Deallocation | Validates terminated task releases allocations back to Available pool | Surface B |
| `TC-T1-27` | R4 | Animated Boot Splash Execution | Validates progress bar rendering `0%` to `100%` before TUI | Surface C |
| `TC-T1-28` | R4 | Multi-Panel TUI Grid Layout | Validates 80x25 dashboard renders all 5 panels with borders | Surface C |
| `TC-T1-29` | R4 | Shell Command: `help` | Validates `help` lists all 9 supported shell commands | Surface C |
| `TC-T1-30` | R4 | Shell Commands: `tasks` & `sched` | Validates task list and MLFQ scheduling statistics display | Surface C |
| `TC-T1-31` | R4 | Shell Commands: `mem` & `deadlock` | Validates heap metrics and Banker safe state output | Surface C |
| `TC-T1-32` | R4 | Shell Command: `web` Portal | Validates simulated network portal (IP 192.168.1.105, Ping 2.14ms, HTTP 200) | Surface C |
| `TC-T1-33` | R5 | Apple Silicon x86 Extradata Check | Validates `VBoxInternal2/EnableX86OnArm = 1` configuration | Surface E |
| `TC-T1-34` | R5 | VM Specification Verification | Validates 128MB RAM, BIOS firmware, x86 architecture | Surface E |
| `TC-T1-35` | R5 | VM Storage ISO Attachment | Validates IDE controller attaches `dist/micro-os.iso` | Surface E |
| `TC-T1-36` | R5 | VM Serial Redirection to Trace | Validates UART1 pipes COM1 to `traces/qemu-boot.log` | Surface E |
| `TC-T1-37` | R5 | Autonomous GUI Launch Parameter | Validates `scripts/run_virtualbox.sh` launches with `--type gui` | Surface E |
| `TC-T1-38` | R6 | Host Unit Test Suite Execution | Validates `tests/kernel_mechanisms.rs` contains $\ge 7$ tests passing | Surface E |
| `TC-T1-39` | R6 | Serial Trace Log Capture Format | Validates trace format matches regex schema | Surface E |
| `TC-T1-40` | R6 | Python Trace Script Execution | Validates `python3 scripts/analyze_trace.py` executes cleanly (exit 0) | Surface E |
| `TC-T1-41` | R6 | Burst Ratio $\Theta$ Metric Extraction | Validates script extracts $\Theta$ for shell (< 0.20) and compute ($\ge 0.20$) | Surface E |
| `TC-T1-42` | R6 | Context Switch & Queue Accounting | Validates switch counting and queue residency distribution | Surface E |

### Tier 2: Boundary & Corner Cases (42 Test Cases)
| Test ID | Area | Test Name | Boundary Condition | Prevented Failure Mode |
|---------|------|-----------|--------------------|------------------------|
| `TC-T2-01` | R1 | ISO Sector Alignment Padding | Exact 2048-byte sector boundary | Truncated boot sector / BIOS read error |
| `TC-T2-02` | R1 | Missing Kernel Binary Detection | Kernel binary absent during image synthesis | Silent generation of empty ISO |
| `TC-T2-03` | R1 | Zero-Length Kernel Binary Handling | Truncated 0-byte kernel binary | Corrupt bootloader image synthesis |
| `TC-T2-04` | R1 | Non-Nightly Rustc Rejection | Build invoked with stable toolchain | Undefined behavior from unsupported features |
| `TC-T2-05` | R1 | VBoxManage Missing Graceful Fallback | Host lacks VBoxManage utility | Hard build failure on headless CI |
| `TC-T2-06` | R2 | VGA Coordinate Bounds Clamping | Rendering at or beyond limits ($col \ge 80, row \ge 25$) | Video RAM buffer overrun |
| `TC-T2-07` | R2 | PS/2 Ring Buffer Saturation | 16-element keyboard buffer flooded with 32 keypresses | Buffer overflow / memory corruption |
| `TC-T2-08` | R2 | Break & Extended Scancode Filter | Key release scancodes ($\ge 0x80$) and `0xE0` prefixes | Garbage injected into shell buffer |
| `TC-T2-09` | R2 | UART Transmitter Full Polling | Rapid continuous string writes over serial | Dropped characters in serial log |
| `TC-T2-10` | R2 | PIC Spurious Interrupt Handling | Spurious IRQ 7 (vec 39) and IRQ 15 (vec 47) | PIC cascade freeze / hang |
| `TC-T2-11` | R3 | Zero Execution Time Burst Ratio | $t_{\text{exec}} = 0, t_{\text{wait}} = 0$ | Division by zero panic ($\frac{0}{0}$) |
| `TC-T2-12` | R3 | Continuous Compute Burst ($\ge 1000$ms) | Heavy compute task running continuously | Misclassification as interactive |
| `TC-T2-13` | R3 | Exact Quantum Expiration Boundary | Task executes exactly 4ms on Q0 or 8ms on Q1 | Quantum overrun / unfair CPU allocation |
| `TC-T2-14` | R3 | Exact 50ms Priority Boost Trigger | Tick progression at boundaries (ticks 4, 5, 6) | Premature or double boost |
| `TC-T2-15` | R3 | Empty Ready Queue Scheduling Tick | 0 ready tasks in all MLFQ queues | Panic on unwrap of empty queue |
| `TC-T2-16` | R3 | Identical Priority Multi-Task Ties | 5 identical interactive tasks added simultaneously | Task starvation / unfair ordering |
| `TC-T2-17` | R3 | Exact 64 KiB Last-Byte Allocation | Allocations totaling exactly 65,536 bytes | Off-by-one heap boundary error |
| `TC-T2-18` | R3 | Out-of-Memory (OOM) Rejection | Allocation request of 65,537 bytes | Buffer overflow beyond heap boundary |
| `TC-T2-19` | R3 | Zero-Byte Allocation Request | Allocation of size 0 bytes | Unbounded pointer advancement |
| `TC-T2-20` | R3 | Non-Power-of-2 Alignment Rejection | Alignment parameter is 3, 7, etc. | Misaligned memory access / CPU fault |
| `TC-T2-21` | R3 | Large Alignment Padding Boundary | 1-byte allocation with 4096-byte alignment | Heap overflow on padding |
| `TC-T2-22` | R3 | Micro-Allocation Saturation | 65,536 single-byte allocations | Metadata overhead corruption |
| `TC-T2-23` | R3 | Banker Request Exceeding Need | Process requests more than declared Need | Process exceeding declared claim contract |
| `TC-T2-24` | R3 | Banker Request Exceeding Available | Process requests more than system Available pool | Negative resource count / underflow |
| `TC-T2-25` | R3 | Circular Wait Deadlock Inducing Request | Request would eliminate all safe sequences | Deadlock state entry |
| `TC-T2-26` | R3 | Zero Available Resources Matrix | System Available vector is all zeros | False positive safety claim |
| `TC-T2-27` | R3 | Full Demand Allocation Termination | Process allocation reaches maximum demand | Resource leak on task termination |
| `TC-T2-28` | R4 | Shell Command Buffer Overflow | Keystroke sequence exceeding 128 characters | Buffer overflow / stack smashing |
| `TC-T2-29` | R4 | Shell Empty Command Input | User presses Enter without typing characters | Panic on empty string indexing |
| `TC-T2-30` | R4 | Shell Whitespace-Only Command Input | User enters spaces and tabs | Phantom command execution |
| `TC-T2-31` | R4 | Shell Interstitial Space Normalization | Spaces surrounding valid command (`  mem   `) | Tokenizer failure on command lookup |
| `TC-T2-32` | R4 | Shell Unrecognized Arbitrary String | User enters arbitrary non-existent command | Shell crash on unknown command |
| `TC-T2-33` | R5 | Pre-Existing VM State Collision | VM `TitaniumMicroOS` already registered | Script failure on name collision |
| `TC-T2-34` | R5 | Missing ISO File Path Handling | `dist/micro-os.iso` missing before attach | Boot failure with empty drive |
| `TC-T2-35` | R5 | Missing `traces/` Directory Handling | Log directory does not exist | Serial redirection pipe failure |
| `TC-T2-36` | R5 | Duplicate Storage Controller Addition | IDE controller already exists on VM | Script abort on duplicate controller error |
| `TC-T2-37` | R5 | Clean Forced Poweroff of Stuck VM | VM process still running from prior session | Lock contention on VM files |
| `TC-T2-38` | R6 | Empty Serial Trace Log File | `traces/qemu-boot.log` is 0 bytes | Python unhandled exception (`IndexError`) |
| `TC-T2-39` | R6 | Corrupted Non-ASCII Log Bytes | Serial log contains raw binary bytes | Python `UnicodeDecodeError` crash |
| `TC-T2-40` | R6 | Truncated / Incomplete Log Lines | Log file ends abruptly mid-line | Regex group match exception |
| `TC-T2-41` | R6 | Single-Event Trace Log | Log file contains exactly one valid line | Statistics division by zero |
| `TC-T2-42` | R6 | High-Volume Log Performance (100k) | Synthetic 100,000-line serial log file | Performance bottleneck / out-of-memory |

### Tier 3: Cross-Feature Combinations (12 Test Cases)
| Test ID | Combined Features | Interaction Scenario | Multi-Step Flow & Expected Outcome |
|---------|-------------------|----------------------|-----------------------------------|
| `TC-T3-01` | MLFQ Scheduler + Bump Allocator | Dynamic task spawning & heap allocation | Tasks allocate memory during quantum; heap tracks 16 KiB without corruption. |
| `TC-T3-02` | Banker's Algorithm + Task MLFQ | Multi-task resource competition during quanta | Safe requests granted, unsafe blocked; system never deadlocks. |
| `TC-T3-03` | Shell Commands + Live TUI Dashboard | Shell commands trigger dashboard re-render | `mem` and `sched` commands synchronously update TUI panels. |
| `TC-T3-04` | Simulated `web` + TUI Telemetry | `web` command activates simulated portal | Screen displays simulated web portal & network telemetry banner. |
| `TC-T3-05` | PIT 100 Hz Timer + MLFQ Priority Boost | Timer interrupts drive scheduler boost interval | Tasks in Q1/Q2 systematically promoted to Q0 every 5th tick (50ms). |
| `TC-T3-06` | SpinLock Sync + Preemptive MLFQ | Tasks competing on SpinLock across preemption | Protected counter updated with atomic pause; zero data corruption. |
| `TC-T3-07` | PS/2 Keyboard + Shell REPL | Keyboard scancodes drive interactive commands | Scancode translation pushes to ring buffer; shell pops and executes. |
| `TC-T3-08` | UART Serial + IDT Exception Handling | CPU exception triggers serial diagnostic dump | Formatted register dump written to COM1; kernel halts safely. |
| `TC-T3-09` | PIT Boot Timer + Splash + TUI Transition | Progress bar transitions cleanly to dashboard | Splash progress reaches 100% and transitions to 80x25 TUI without tearing. |
| `TC-T3-10` | Allocator Squeeze + Banker Allocation | Heap exhaustion interacting with Banker claims | Allocator exhaustion denies resource safely; state rolled back cleanly. |
| `TC-T3-11` | UART Serial Stream + Python Analyzer | Kernel serial output matches Python regex schema | 100% of generated log lines match regex; metrics table generated. |
| `TC-T3-12` | Build Pipeline + VirtualBox Boot | Build artifacts drive automated VM execution | ISO compiled by `build.rs` provisions and boots in VirtualBox window. |

### Tier 4: Real-World Application Scenarios (6 Scenarios)
| Scenario ID | Name | Scenario Objective & Workload Flow | Acceptance SLA |
|-------------|------|-----------------------------------|----------------|
| `TC-T4-01` | Heterogeneous Multi-Tenant Batch & Interactive | 4 tasks (`shell`, `logger`, `worker`, `idle`) over 100 rounds with periodic 50ms boosts | $\Theta_{\text{shell}} < 0.20$, $\Theta_{\text{worker}} \ge 0.20$, 0 starvation, $\ge 50$ switches. |
| `TC-T4-02` | High-Contention Deadlock Avoidance | 4 processes compete for CPU, Memory, I/O with circular wait induction | `is_safe() == true` continuously, unsafe requests rejected, zero deadlocks. |
| `TC-T4-03` | Complete User Session: Boot, Shell & Audit | BIOS boot -> Splash 2s -> TUI -> 9 shell commands including `web` -> clear | All 9 commands format correctly; `web` displays mock network portal. |
| `TC-T4-04` | Full Trace Analysis & Metrics Pipeline | VM serial trace captured to `traces/qemu-boot.log` -> parsed by Python script | Exit 0, response/wait/burst metrics extracted, matches scheduler counters. |
| `TC-T4-05` | Heap Saturation, OOM Recovery & Resilience | Allocator filled to exact 64 KiB capacity -> OOM handled -> `mem` command checks | Allocator prevents memory corruption; system remains 100% responsive. |
| `TC-T4-06` | Autonomous Cold-Boot Build & VM Execution | `cargo test` (7+ tests) -> `cargo build` (ISO) -> `scripts/run_virtualbox.sh` | Clean build, valid ISO 9660, VM boots with visible GUI window. |
