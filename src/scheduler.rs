//! 3-Queue Workload-Aware MLFQ Scheduler for Titanium Micro-OS.
//!
//! Quantum hierarchy: Q0 = 4ms, Q1 = 8ms, Q2 = 32ms.
//! Periodic priority boost moves all ready tasks to Q0 to prevent starvation.
//! Research Track R4: Workload classification via burst ratio Theta.

use std::collections::VecDeque;
use crate::task::{Task, TaskState};

pub const Q0_QUANTUM: u64 = 4;
pub const Q1_QUANTUM: u64 = 8;
pub const Q2_QUANTUM: u64 = 32;
pub const DEFAULT_BOOST_INTERVAL: u64 = 50;

#[derive(Debug, Clone, PartialEq)]
pub struct ScheduleEvent {
    pub tick: u64,
    pub task_id: usize,
    pub task_name: &'static str,
    pub queue: usize,
    pub remaining_steps: usize,
    pub completed: bool,
    pub burst_ratio: f64,
    pub boosted: bool,
}

pub struct MlfqScheduler {
    pub queues: [VecDeque<Task>; 3],
    pub quantums_ms: [u64; 3],
    pub tick_count: u64,
    pub boost_interval: u64,
    pub switches: usize,
    pub completed: usize,
    pub boost_count: usize,
    last_task_id: Option<usize>,
}

impl MlfqScheduler {
    /// Creates a new `MlfqScheduler` with default 3-queue MLFQ configuration.
    pub fn new() -> Self {
        Self {
            queues: [VecDeque::new(), VecDeque::new(), VecDeque::new()],
            quantums_ms: [Q0_QUANTUM, Q1_QUANTUM, Q2_QUANTUM],
            tick_count: 0,
            boost_interval: DEFAULT_BOOST_INTERVAL,
            switches: 0,
            completed: 0,
            boost_count: 0,
            last_task_id: None,
        }
    }

    /// Returns the total number of active tasks across all 3 ready queues.
    pub fn total_tasks(&self) -> usize {
        self.queues.iter().map(|q| q.len()).sum()
    }

    /// Returns true if all ready queues are empty.
    pub fn is_empty(&self) -> bool {
        self.queues.iter().all(|q| q.is_empty())
    }

    /// Adds a new task into the highest priority queue (Q0).
    pub fn add_task(&mut self, mut task: Task) {
        task.queue = 0;
        task.state = TaskState::Ready;
        task.time_in_quantum = 0;
        self.queues[0].push_back(task);
    }

    /// Boosts all tasks from Q1 and Q2 to Q0 to prevent starvation.
    pub fn priority_boost(&mut self) -> usize {
        let mut count = 0;
        while let Some(mut task) = self.queues[1].pop_front() {
            task.queue = 0;
            task.time_in_quantum = 0;
            task.state = TaskState::Ready;
            self.queues[0].push_back(task);
            count += 1;
        }
        while let Some(mut task) = self.queues[2].pop_front() {
            task.queue = 0;
            task.time_in_quantum = 0;
            task.state = TaskState::Ready;
            self.queues[0].push_back(task);
            count += 1;
        }
        self.boost_count += 1;
        count
    }

    /// Executes a single simulation step/tick.
    pub fn schedule_once(&mut self) -> Option<ScheduleEvent> {
        if self.is_empty() {
            return None;
        }

        self.tick_count += 1;

        // 1. Periodic Priority Boost
        let mut boosted = false;
        #[allow(clippy::manual_is_multiple_of)]
        if self.boost_interval > 0 && self.tick_count % self.boost_interval == 0 {
            self.priority_boost();
            boosted = true;
        }

        // 2. Select next task from highest priority non-empty queue
        let queue_idx = if !self.queues[0].is_empty() {
            0
        } else if !self.queues[1].is_empty() {
            1
        } else if !self.queues[2].is_empty() {
            2
        } else {
            return None;
        };

        let mut current_task = self.queues[queue_idx].pop_front().unwrap();

        // 3. Track context switches
        if self.last_task_id != Some(current_task.id) {
            self.switches += 1;
            self.last_task_id = Some(current_task.id);
        }

        // 4. Update time accounting
        current_task.state = TaskState::Running;
        current_task.exec_time += 1;
        current_task.time_in_quantum += 1;
        current_task.steps = current_task.steps.saturating_sub(1);

        // Increment wait time for all other ready tasks in queues
        for q in &mut self.queues {
            for t in q.iter_mut() {
                t.wait_time += 1;
            }
        }

        let completed = current_task.steps == 0;
        let remaining_steps = current_task.steps;
        let burst_ratio = current_task.burst_ratio();
        let task_id = current_task.id;
        let task_name = current_task.name;

        // 5. Completion or requeueing
        if completed {
            self.completed += 1;
        } else {
            let quantum = self.quantums_ms[queue_idx];
            if current_task.time_in_quantum >= quantum {
                // Quantum expired: reset quantum timer and demote
                current_task.time_in_quantum = 0;
                current_task.state = TaskState::Ready;
                current_task.update_classification();

                match queue_idx {
                    0 => {
                        // Theta < 0.20 stays in Q0 (Interactive bias)
                        if current_task.burst_ratio() < 0.20 {
                            current_task.queue = 0;
                            self.queues[0].push_back(current_task);
                        } else {
                            current_task.queue = 1;
                            self.queues[1].push_back(current_task);
                        }
                    }
                    1 => {
                        current_task.queue = 2;
                        self.queues[2].push_back(current_task);
                    }
                    _ => {
                        current_task.queue = 2;
                        self.queues[2].push_back(current_task);
                    }
                }
            } else {
                // Quantum unexpired: retain task at front of current queue
                current_task.state = TaskState::Ready;
                self.queues[queue_idx].push_front(current_task);
            }
        }

        Some(ScheduleEvent {
            tick: self.tick_count,
            task_id,
            task_name,
            queue: queue_idx,
            remaining_steps,
            completed,
            burst_ratio,
            boosted,
        })
    }
}

impl Default for MlfqScheduler {
    fn default() -> Self {
        Self::new()
    }
}
