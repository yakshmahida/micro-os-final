//! Bare-Metal 3-Queue Workload-Aware MLFQ Scheduler.
//!
//! Quantum hierarchy: Q0 = 4, Q1 = 8, Q2 = 32.
//! Priority boost: every 5 ticks (50ms at 100 Hz PIT).
//! Static ring buffers for ready queue task indices.

use crate::sync::SpinLock;
use crate::task::{Task, TaskState, MAX_TASKS};

pub const Q0_QUANTUM: u64 = 4;
pub const Q1_QUANTUM: u64 = 8;
pub const Q2_QUANTUM: u64 = 32;
pub const DEFAULT_BOOST_INTERVAL: u64 = 5; // 50ms at 100 Hz PIT

#[derive(Clone, Copy)]
pub struct RingBuffer<const CAP: usize> {
    buffer: [usize; CAP],
    head: usize,
    tail: usize,
    len: usize,
}

impl<const CAP: usize> RingBuffer<CAP> {
    pub const fn new() -> Self {
        Self {
            buffer: [0; CAP],
            head: 0,
            tail: 0,
            len: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn is_full(&self) -> bool {
        self.len >= CAP
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn push_back(&mut self, item: usize) -> bool {
        if self.is_full() {
            return false;
        }
        self.buffer[self.tail] = item;
        self.tail = (self.tail + 1) % CAP;
        self.len += 1;
        true
    }

    pub fn push_front(&mut self, item: usize) -> bool {
        if self.is_full() {
            return false;
        }
        self.head = if self.head == 0 { CAP - 1 } else { self.head - 1 };
        self.buffer[self.head] = item;
        self.len += 1;
        true
    }

    pub fn pop_front(&mut self) -> Option<usize> {
        if self.is_empty() {
            return None;
        }
        let item = self.buffer[self.head];
        self.head = (self.head + 1) % CAP;
        self.len -= 1;
        Some(item)
    }

    pub fn clear(&mut self) {
        self.head = 0;
        self.tail = 0;
        self.len = 0;
    }
}

impl<const CAP: usize> Default for RingBuffer<CAP> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MlfqScheduler {
    pub tasks: [Option<Task>; MAX_TASKS],
    pub queues: [RingBuffer<MAX_TASKS>; 3],
    pub quantums_ms: [u64; 3],
    pub tick_count: u64,
    pub boost_interval: u64,
    pub switches: usize,
    pub completed: usize,
    pub boost_count: usize,
    pub last_task_id: Option<usize>,
}

impl MlfqScheduler {
    pub const fn new() -> Self {
        Self {
            tasks: [None; MAX_TASKS],
            queues: [RingBuffer::new(), RingBuffer::new(), RingBuffer::new()],
            quantums_ms: [Q0_QUANTUM, Q1_QUANTUM, Q2_QUANTUM],
            tick_count: 0,
            boost_interval: DEFAULT_BOOST_INTERVAL,
            switches: 0,
            completed: 0,
            boost_count: 0,
            last_task_id: None,
        }
    }

    pub fn total_tasks(&self) -> usize {
        self.queues[0].len() + self.queues[1].len() + self.queues[2].len()
    }

    pub fn is_empty(&self) -> bool {
        self.total_tasks() == 0
    }

    pub fn get_tasks(&self) -> &[Option<Task>] {
        &self.tasks
    }

    #[allow(clippy::result_unit_err, clippy::needless_range_loop)]
    pub fn add_task(&mut self, mut task: Task) -> Result<usize, ()> {
        let slot = if task.id < MAX_TASKS && self.tasks[task.id].is_none() {
            task.id
        } else {
            let mut free = None;
            for i in 0..MAX_TASKS {
                if self.tasks[i].is_none() {
                    free = Some(i);
                    break;
                }
            }
            free.ok_or(())?
        };

        task.id = slot;
        task.queue = 0;
        task.state = TaskState::Ready;
        task.time_in_quantum = 0;
        self.tasks[slot] = Some(task);
        self.queues[0].push_back(slot);
        Ok(slot)
    }

    pub fn priority_boost(&mut self) -> usize {
        let mut count = 0;
        while let Some(id) = self.queues[1].pop_front() {
            if let Some(t) = self.tasks[id].as_mut() {
                t.queue = 0;
                t.time_in_quantum = 0;
                t.state = TaskState::Ready;
            }
            self.queues[0].push_back(id);
            count += 1;
        }
        while let Some(id) = self.queues[2].pop_front() {
            if let Some(t) = self.tasks[id].as_mut() {
                t.queue = 0;
                t.time_in_quantum = 0;
                t.state = TaskState::Ready;
            }
            self.queues[0].push_back(id);
            count += 1;
        }
        self.boost_count += 1;
        count
    }

    #[allow(clippy::needless_range_loop, clippy::manual_is_multiple_of)]
    pub fn tick(&mut self) -> Option<usize> {
        self.tick_count += 1;

        // 1. Priority Boost every boost_interval ticks (50ms)
        if self.boost_interval > 0 && self.tick_count % self.boost_interval == 0 {
            self.priority_boost();
            crate::serial_println!("[BOOST] tick={} all tasks boosted to Q0", self.tick_count);
        }

        // 2. Select highest priority non-empty queue
        let queue_idx = if !self.queues[0].is_empty() {
            0
        } else if !self.queues[1].is_empty() {
            1
        } else if !self.queues[2].is_empty() {
            2
        } else {
            return None;
        };

        let current_id = self.queues[queue_idx].pop_front()?;

        // 3. Track context switches
        if self.last_task_id != Some(current_id) {
            self.switches += 1;
            self.last_task_id = Some(current_id);
        }

        // 4. Update wait time for other ready tasks
        for slot in 0..MAX_TASKS {
            if slot != current_id {
                if let Some(t) = self.tasks[slot].as_mut() {
                    if t.state == TaskState::Ready {
                        t.wait_time += 1;
                    }
                }
            }
        }

        // 5. Time accounting for current task
        let current_task = self.tasks[current_id].as_mut()?;
        current_task.state = TaskState::Running;
        current_task.exec_time += 1;
        current_task.time_in_quantum += 1;
        current_task.steps = current_task.steps.saturating_sub(1);

        let completed = current_task.steps == 0;
        let (whole, frac) = current_task.burst_ratio_parts();

        // 6. Emit serial trace log
        crate::serial_println!(
            "[SCHED] tick={} task={} ({}) Q={} state=Running exec={}ms wait={}ms yield={}ms Θ={}.{:04}",
            self.tick_count,
            current_task.id,
            current_task.name,
            queue_idx,
            current_task.exec_time,
            current_task.wait_time,
            current_task.yield_time,
            whole,
            frac
        );

        // 7. Completion or queue demotion/retention
        if completed {
            current_task.state = TaskState::Terminated;
            self.completed += 1;
        } else {
            let quantum = self.quantums_ms[queue_idx];
            if current_task.time_in_quantum >= quantum {
                current_task.time_in_quantum = 0;
                current_task.state = TaskState::Ready;
                current_task.update_classification();

                match queue_idx {
                    0 => {
                        if current_task.burst_ratio_pct() < 20 {
                            current_task.queue = 0;
                            self.queues[0].push_back(current_id);
                        } else {
                            current_task.queue = 1;
                            self.queues[1].push_back(current_id);
                        }
                    }
                    1 => {
                        current_task.queue = 2;
                        self.queues[2].push_back(current_id);
                    }
                    _ => {
                        current_task.queue = 2;
                        self.queues[2].push_back(current_id);
                    }
                }
            } else {
                current_task.state = TaskState::Ready;
                self.queues[queue_idx].push_front(current_id);
            }
        }

        Some(current_id)
    }
}

impl Default for MlfqScheduler {
    fn default() -> Self {
        Self::new()
    }
}

pub static SCHEDULER: SpinLock<MlfqScheduler> = SpinLock::new(MlfqScheduler::new());

pub fn tick() -> Option<usize> {
    if let Some(mut sched) = SCHEDULER.try_lock() {
        sched.tick()
    } else {
        None
    }
}
