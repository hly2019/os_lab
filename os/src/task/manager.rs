//! Implementation of [`TaskManager`]
//!
//! It is only used to manage processes and schedule process based on ready queue.
//! Other CPU process monitoring functions are in Processor.


use super::TaskControlBlock;
use crate::sync::UPSafeCell;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use lazy_static::*;

pub struct TaskManager {
    ready_queue: VecDeque<Arc<TaskControlBlock>>,
}

// YOUR JOB: FIFO->Stride
/// A simple FIFO scheduler.
impl TaskManager {
    pub fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
        }
    }
    /// Add process back to ready queue
    pub fn add(&mut self, task: Arc<TaskControlBlock>) {
        self.ready_queue.push_back(task);
    }
    /// Take a process out of the ready queue
    pub fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        let mut ret = 0;
        let mut stride = usize::MAX;
        let mut priority = 0;
        for it in 0..self.ready_queue.len() {
            if self.ready_queue.get(it).unwrap().inner_exclusive_access().stride <= stride {
                stride = self.ready_queue.get(it).unwrap().inner_exclusive_access().stride;
                ret = it;
                priority = self.ready_queue.get(it).unwrap().inner_exclusive_access().priority;
            }
        }
        let BigStrike: isize = 10000000;

        self.ready_queue.get(ret).unwrap().inner_exclusive_access().stride
         += (BigStrike / priority) as usize;
        self.ready_queue.swap(0, ret);
        self.ready_queue.pop_front()
    }
}

lazy_static! {
    /// TASK_MANAGER instance through lazy_static!
    pub static ref TASK_MANAGER: UPSafeCell<TaskManager> =
        unsafe { UPSafeCell::new(TaskManager::new()) };
}

pub fn add_task(task: Arc<TaskControlBlock>) {
    TASK_MANAGER.exclusive_access().add(task);
}

pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    TASK_MANAGER.exclusive_access().fetch()
}
