//! Bare-Metal Task Representation & Fixed-Point Integer Burst Ratio.
//! CS2001 OS Capstone | Research Track R4 (Titanium Micro-OS)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Ready,
    Running,
    Blocked,
    Terminated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskClass {
    Interactive, // Class C1: Theta < 0.20 (Theta_pct < 20)
    Compute,     // Class C2: Theta >= 0.20 (Theta_pct >= 20)
}

pub const MAX_TASKS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Task {
    pub id: usize,
    pub name: &'static str,
    pub state: TaskState,
    pub class: TaskClass,
    pub steps: usize,
    pub exec_time: u64,
    pub wait_time: u64,
    pub yield_time: u64,
    pub queue: usize,
    pub time_in_quantum: u64,
}

impl Task {
    pub const fn new(id: usize, name: &'static str, class: TaskClass, steps: usize) -> Self {
        Self {
            id,
            name,
            state: TaskState::Ready,
            class,
            steps,
            exec_time: 0,
            wait_time: 0,
            yield_time: 0,
            queue: 0,
            time_in_quantum: 0,
        }
    }

    /// Fixed-point integer burst ratio percentage in range 0..=100:
    /// Theta_pct = (t_exec * 100) / (t_exec + t_wait + t_yield)
    /// Guarantees zero SSE / floating-point instructions in bare metal.
    #[allow(clippy::manual_checked_ops)]
    pub fn burst_ratio_pct(&self) -> u32 {
        let total = self.exec_time + self.wait_time + self.yield_time;
        if total == 0 {
            match self.class {
                TaskClass::Interactive => 0,
                TaskClass::Compute => 50,
            }
        } else {
            ((self.exec_time * 100) / total) as u32
        }
    }

    /// Classification using 20% (0.20) threshold.
    pub fn calculated_class(&self) -> TaskClass {
        if self.burst_ratio_pct() < 20 {
            TaskClass::Interactive
        } else {
            TaskClass::Compute
        }
    }

    pub fn update_classification(&mut self) {
        self.class = self.calculated_class();
    }

    pub fn is_terminated(&self) -> bool {
        self.state == TaskState::Terminated || self.steps == 0
    }

    /// Computes whole and 4-digit fractional components for formatting `Θ=whole.frac`
    /// without any floating-point instructions.
    #[allow(clippy::manual_checked_ops)]
    pub fn burst_ratio_parts(&self) -> (u64, u64) {
        let total = self.exec_time + self.wait_time + self.yield_time;
        if total == 0 {
            match self.class {
                TaskClass::Interactive => (0, 0),
                TaskClass::Compute => (0, 5000),
            }
        } else {
            let scaled = (self.exec_time * 10_000 + total / 2) / total;
            (scaled / 10_000, scaled % 10_000)
        }
    }
}
