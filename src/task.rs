//! Task representation and burst ratio calculation for Titanium Micro-OS.
//! Research Track R4: Workload classification via burst ratio Theta.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Ready,
    Running,
    Blocked,
    Terminated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskClass {
    Interactive, // Class C1: Theta < 0.20
    Compute,     // Class C2: Theta >= 0.20
}

#[derive(Debug, Clone, PartialEq)]
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
    pub fn new(id: usize, name: &'static str, class: TaskClass, steps: usize) -> Self {
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

    /// Calculates burst ratio Theta = t_exec / (t_exec + t_wait + t_yield)
    /// Safely handles 0 total without division by zero.
    pub fn burst_ratio(&self) -> f64 {
        let total = self.exec_time + self.wait_time + self.yield_time;
        if total == 0 {
            match self.class {
                TaskClass::Interactive => 0.0,
                TaskClass::Compute => 0.5,
            }
        } else {
            self.exec_time as f64 / total as f64
        }
    }

    /// Workload classification based on the 0.20 threshold.
    pub fn calculated_class(&self) -> TaskClass {
        if self.burst_ratio() < 0.20 {
            TaskClass::Interactive
        } else {
            TaskClass::Compute
        }
    }

    /// Updates workload classification based on the 0.20 threshold.
    pub fn update_classification(&mut self) {
        self.class = self.calculated_class();
    }

    pub fn is_terminated(&self) -> bool {
        self.state == TaskState::Terminated || self.steps == 0
    }
}
