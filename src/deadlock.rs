//! Dijkstra's Banker's Algorithm for deadlock avoidance in Titanium Micro-OS.
//!
//! Provides safety state analysis, safe sequence determination, and
//! transactional resource request evaluation with automatic rollback.

#![allow(clippy::needless_range_loop)]

/// Snapshot of the Banker's Algorithm status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankerStatus {
    pub is_safe: bool,
    pub safe_sequence: Vec<usize>,
    pub available: Vec<u32>,
}

/// Dijkstra's Banker's Algorithm for $N$ processes and $R$ resource types.
#[derive(Debug, Clone)]
pub struct BankersAlgorithm {
    pub n: usize,
    pub r: usize,
    pub available: Vec<u32>,
    pub allocation: Vec<Vec<u32>>,
    pub max_demand: Vec<Vec<u32>>,
}

impl BankersAlgorithm {
    /// Creates a new `BankersAlgorithm` instance for $N$ processes and $R$ resource types.
    pub fn new(n: usize, r: usize) -> Self {
        Self {
            n,
            r,
            available: vec![0; r],
            allocation: vec![vec![0; r]; n],
            max_demand: vec![vec![0; r]; n],
        }
    }

    /// Sets the available resource vector.
    pub fn set_available(&mut self, available: &[u32]) {
        assert_eq!(available.len(), self.r, "Available vector length mismatch");
        self.available.copy_from_slice(available);
    }

    /// Sets the current allocation vector for process `pid`.
    pub fn set_allocation(&mut self, pid: usize, alloc: &[u32]) {
        assert!(pid < self.n, "Process ID out of bounds");
        assert_eq!(alloc.len(), self.r, "Allocation vector length mismatch");
        self.allocation[pid].copy_from_slice(alloc);
    }

    /// Sets the maximum resource demand vector for process `pid`.
    pub fn set_max(&mut self, pid: usize, max: &[u32]) {
        assert!(pid < self.n, "Process ID out of bounds");
        assert_eq!(max.len(), self.r, "Max demand vector length mismatch");
        self.max_demand[pid].copy_from_slice(max);
    }

    /// Computes the remaining resource need for process `pid` and resource type `j`.
    pub fn need(&self, pid: usize, j: usize) -> u32 {
        self.max_demand[pid][j].saturating_sub(self.allocation[pid][j])
    }

    /// Returns the full $N \times R$ Need matrix.
    pub fn get_need_matrix(&self) -> Vec<Vec<u32>> {
        let mut need = vec![vec![0; self.r]; self.n];
        for i in 0..self.n {
            for j in 0..self.r {
                need[i][j] = self.need(i, j);
            }
        }
        need
    }

    /// Computes a safe execution sequence of process IDs using Dijkstra's Banker's Algorithm.
    /// Returns `Some(sequence)` if the system is safe, or `None` if in an unsafe state.
    pub fn safe_sequence(&self) -> Option<Vec<usize>> {
        let mut work = self.available.clone();
        let mut finish = vec![false; self.n];
        let mut safe_seq = Vec::with_capacity(self.n);

        for _ in 0..self.n {
            let mut found = false;
            for p in 0..self.n {
                if !finish[p] {
                    let mut can_allocate = true;
                    for j in 0..self.r {
                        let need_j = self.need(p, j);
                        if need_j > work[j] {
                            can_allocate = false;
                            break;
                        }
                    }
                    if can_allocate {
                        for j in 0..self.r {
                            work[j] += self.allocation[p][j];
                        }
                        finish[p] = true;
                        safe_seq.push(p);
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                break;
            }
        }

        if safe_seq.len() == self.n {
            Some(safe_seq)
        } else {
            None
        }
    }

    /// Returns `true` if current state is safe from deadlock.
    pub fn is_safe(&self) -> bool {
        self.safe_sequence().is_some()
    }

    /// Evaluates a resource request by process `pid`.
    ///
    /// If the request cannot be granted or leads to an unsafe state,
    /// changes are immediately rolled back and `false` is returned.
    /// If safe, allocation is confirmed and `true` is returned.
    pub fn request_resources(&mut self, pid: usize, req: &[u32]) -> bool {
        if pid >= self.n || req.len() != self.r {
            return false;
        }

        // 1. Claim check: req <= Need[pid]
        for j in 0..self.r {
            let need_j = self.need(pid, j);
            if req[j] > need_j {
                return false;
            }
        }

        // 2. Availability check: req <= Available
        for j in 0..self.r {
            if req[j] > self.available[j] {
                return false;
            }
        }

        // 3. Tentative allocation
        for j in 0..self.r {
            self.available[j] -= req[j];
            self.allocation[pid][j] += req[j];
        }

        // 4. Safety evaluation
        if self.is_safe() {
            true
        } else {
            // Rollback tentative changes
            for j in 0..self.r {
                self.available[j] += req[j];
                self.allocation[pid][j] -= req[j];
            }
            false
        }
    }

    /// Releases resources previously held by process `pid`.
    pub fn release_resources(&mut self, pid: usize, rel: &[u32]) -> bool {
        if pid >= self.n || rel.len() != self.r {
            return false;
        }
        for j in 0..self.r {
            if rel[j] > self.allocation[pid][j] {
                return false;
            }
        }
        for j in 0..self.r {
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
            safe_sequence: seq.unwrap_or_default(),
            available: self.available.clone(),
        }
    }
}
