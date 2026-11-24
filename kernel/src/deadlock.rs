//! Dijkstra's Banker's Algorithm for deadlock avoidance in Titanium Micro-OS.
//!
//! Provides safety state analysis, safe sequence determination, and
//! transactional resource request evaluation with automatic rollback.
//! Statically bounded for N=4 processes and R=3 resource types in #![no_std].

use crate::sync::SpinLock;

pub const NUM_PROCESSES: usize = 4;
pub const NUM_RESOURCES: usize = 3;

pub const PROCESS_NAMES: [&str; NUM_PROCESSES] = ["shell", "logger", "worker", "idle"];
pub const RESOURCE_NAMES: [&str; NUM_RESOURCES] = ["CPU", "RAM", "I/O"];

/// Snapshot of the Banker's Algorithm status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BankerStatus {
    pub is_safe: bool,
    pub safe_sequence: [usize; NUM_PROCESSES],
    pub available: [u32; NUM_RESOURCES],
}

/// Dijkstra's Banker's Algorithm for fixed 4 processes and 3 resource types.
#[derive(Debug, Clone, Copy)]
pub struct BankersAlgorithm {
    pub available: [u32; NUM_RESOURCES],
    pub allocation: [[u32; NUM_RESOURCES]; NUM_PROCESSES],
    pub max_demand: [[u32; NUM_RESOURCES]; NUM_PROCESSES],
}

impl BankersAlgorithm {
    /// Creates a new `BankersAlgorithm` pre-configured to the default safe state.
    pub const fn new() -> Self {
        Self {
            available: [3, 3, 2],
            allocation: [
                [1, 0, 0], // P0: shell
                [0, 1, 0], // P1: logger
                [0, 0, 1], // P2: worker
                [1, 1, 0], // P3: idle
            ],
            max_demand: [
                [3, 2, 2], // P0: shell
                [2, 2, 2], // P1: logger
                [2, 1, 2], // P2: worker
                [2, 2, 1], // P3: idle
            ],
        }
    }

    /// Creates an empty instance with all zeroes.
    pub const fn empty() -> Self {
        Self {
            available: [0; NUM_RESOURCES],
            allocation: [[0; NUM_RESOURCES]; NUM_PROCESSES],
            max_demand: [[0; NUM_RESOURCES]; NUM_PROCESSES],
        }
    }

    /// Sets the available resource vector.
    pub fn set_available(&mut self, available: [u32; NUM_RESOURCES]) {
        self.available = available;
    }

    /// Sets the current allocation vector for process `pid`.
    pub fn set_allocation(&mut self, pid: usize, alloc: [u32; NUM_RESOURCES]) {
        if pid < NUM_PROCESSES {
            self.allocation[pid] = alloc;
        }
    }

    /// Sets the maximum resource demand vector for process `pid`.
    pub fn set_max(&mut self, pid: usize, max: [u32; NUM_RESOURCES]) {
        if pid < NUM_PROCESSES {
            self.max_demand[pid] = max;
        }
    }

    /// Computes the remaining resource need for process `pid` and resource type `j`.
    pub fn need(&self, pid: usize, j: usize) -> u32 {
        if pid < NUM_PROCESSES && j < NUM_RESOURCES {
            self.max_demand[pid][j].saturating_sub(self.allocation[pid][j])
        } else {
            0
        }
    }

    /// Returns the full 4x3 Need matrix.
    pub fn get_need_matrix(&self) -> [[u32; NUM_RESOURCES]; NUM_PROCESSES] {
        let mut need = [[0u32; NUM_RESOURCES]; NUM_PROCESSES];
        let mut p = 0;
        while p < NUM_PROCESSES {
            let mut r = 0;
            while r < NUM_RESOURCES {
                need[p][r] = self.need(p, r);
                r += 1;
            }
            p += 1;
        }
        need
    }

    /// Computes a safe execution sequence using Dijkstra's Banker's Algorithm.
    /// Returns `Some([usize; 4])` if safe, or `None` if in an unsafe state.
    #[allow(clippy::needless_range_loop)]
    pub fn safe_sequence(&self) -> Option<[usize; NUM_PROCESSES]> {
        let mut work = self.available;
        let mut finish = [false; NUM_PROCESSES];
        let mut safe_seq = [0usize; NUM_PROCESSES];
        let mut count = 0;

        for _ in 0..NUM_PROCESSES {
            let mut found = false;
            for p in 0..NUM_PROCESSES {
                if !finish[p] {
                    let mut can_allocate = true;
                    for j in 0..NUM_RESOURCES {
                        if self.need(p, j) > work[j] {
                            can_allocate = false;
                            break;
                        }
                    }
                    if can_allocate {
                        for j in 0..NUM_RESOURCES {
                            work[j] += self.allocation[p][j];
                        }
                        finish[p] = true;
                        safe_seq[count] = p;
                        count += 1;
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                break;
            }
        }

        if count == NUM_PROCESSES {
            Some(safe_seq)
        } else {
            None
        }
    }

    /// Returns `true` if the current state is safe from deadlock.
    pub fn is_safe(&self) -> bool {
        self.safe_sequence().is_some()
    }

    /// Evaluates a resource request by process `pid`.
    ///
    /// If request cannot be granted or leads to an unsafe state,
    /// changes are immediately rolled back and `false` is returned.
    /// If safe, allocation is confirmed and `true` is returned.
    #[allow(clippy::needless_range_loop)]
    pub fn request_resources(&mut self, pid: usize, req: &[u32; NUM_RESOURCES]) -> bool {
        if pid >= NUM_PROCESSES {
            return false;
        }

        // 1. Claim check: req[j] <= Need[pid][j]
        for j in 0..NUM_RESOURCES {
            if req[j] > self.need(pid, j) {
                return false;
            }
        }

        // 2. Availability check: req[j] <= Available[j]
        for j in 0..NUM_RESOURCES {
            if req[j] > self.available[j] {
                return false;
            }
        }

        // 3. Tentative allocation
        for j in 0..NUM_RESOURCES {
            self.available[j] -= req[j];
            self.allocation[pid][j] += req[j];
        }

        // 4. Safety evaluation
        if self.is_safe() {
            true
        } else {
            // Rollback tentative changes
            for j in 0..NUM_RESOURCES {
                self.available[j] += req[j];
                self.allocation[pid][j] -= req[j];
            }
            false
        }
    }

    /// Releases resources previously held by process `pid`.
    pub fn release_resources(&mut self, pid: usize, rel: &[u32; NUM_RESOURCES]) -> bool {
        if pid >= NUM_PROCESSES {
            return false;
        }
        for j in 0..NUM_RESOURCES {
            if rel[j] > self.allocation[pid][j] {
                return false;
            }
        }
        for j in 0..NUM_RESOURCES {
            self.allocation[pid][j] -= rel[j];
            self.available[j] += rel[j];
        }
        true
    }

    /// Generates a status summary of Banker's state for dashboard/shell reporting.
    pub fn status(&self) -> BankerStatus {
        let seq = self.safe_sequence();
        BankerStatus {
            is_safe: seq.is_some(),
            safe_sequence: seq.unwrap_or([0; NUM_PROCESSES]),
            available: self.available,
        }
    }
}

impl Default for BankersAlgorithm {
    fn default() -> Self {
        Self::new()
    }
}

/// Global system Banker's Algorithm instance wrapped in a SpinLock.
pub static BANKER: SpinLock<BankersAlgorithm> = SpinLock::new(BankersAlgorithm::new());

pub fn is_safe() -> bool {
    BANKER.lock().is_safe()
}

pub fn safe_sequence() -> Option<[usize; NUM_PROCESSES]> {
    BANKER.lock().safe_sequence()
}

pub fn request_resources(pid: usize, req: &[u32; NUM_RESOURCES]) -> bool {
    BANKER.lock().request_resources(pid, req)
}

pub fn release_resources(pid: usize, rel: &[u32; NUM_RESOURCES]) -> bool {
    BANKER.lock().release_resources(pid, rel)
}

pub fn status() -> BankerStatus {
    BANKER.lock().status()
}
