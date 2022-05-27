use super::UPSafeCell;
use crate::task::{TaskControlBlock, current_process};
use crate::task::{add_task, current_task};
use crate::task::{block_current_and_run_next, suspend_current_and_run_next};
use alloc::{collections::VecDeque, sync::Arc};

pub trait Mutex: Sync + Send {
    fn lock(&self);
    fn unlock(&self);
    fn islocked(&self) -> usize;
}

pub struct MutexSpin {
    // locked: UPSafeCell<bool>,
    inner: UPSafeCell<MutexSpinInner>,

}
pub struct MutexSpinInner {
    locked: bool,
    // wait_queue: VecDeque<Arc<TaskControlBlock>>,
    id: usize,
}
impl MutexSpin {
    pub fn new(id: usize) -> Self {
        let process = current_process();
        let mut process_inner = process.inner_exclusive_access();
        let tasks = &mut process_inner.tasks;
        for id in 0..tasks.len() {
            // &mut tasks[id].unwrap().inner_exclusive_access().mtx_allocation;
            let current_task = tasks[id].as_ref();
            if let Some(current_task) = current_task {
                let allocation = &mut current_task.inner_exclusive_access().mtx_allocation;
                while allocation.len() < id + 1 { // expand allocation
                    allocation.push(0);
                }
                let need = &mut current_task.inner_exclusive_access().mtx_need;
                while need.len() < id + 1 { // expand need
                    need.push(0);
                }
            }
        }
        Self {
            inner: unsafe {
                UPSafeCell::new(MutexSpinInner {
                    locked: false,
                    id: id,
                })
            },
        }
    }
}

impl Mutex for MutexSpin {
    fn islocked(&self) -> usize {
        if self.inner.exclusive_access().locked {
            return 0;
        }
        else {
            return 1;
        }
    }
    fn lock(&self) {
        loop {
            let mut mutex_inner = self.inner.exclusive_access();
            // let mut locked = self.locked.exclusive_access();
            if mutex_inner.locked {
                drop(mutex_inner);
                let cur_task = current_task().unwrap();
                let need = &mut cur_task.inner_exclusive_access().mtx_need;
                let mtx_id = self.inner.exclusive_access().id;
                need[mtx_id] = 1; // need the lock
                suspend_current_and_run_next();
                continue;
            } else {
                let cur_task = current_task().unwrap();
                let need = &mut cur_task.inner_exclusive_access().mtx_need;
                let mtx_id = self.inner.exclusive_access().id;
                need[mtx_id] = 0; // no longer need the lock 
                let allocation = &mut cur_task.inner_exclusive_access().mtx_allocation;
                allocation[mtx_id] = 1; // alloc current lock to the thread
                mutex_inner.locked = true;
                return;
            }
        }
    }

    fn unlock(&self) {
        let mut mutex_inner = self.inner.exclusive_access();

        // let mut locked = self.locked.exclusive_access();
        mutex_inner.locked = false;
    }
}

pub struct MutexBlocking {
    inner: UPSafeCell<MutexBlockingInner>,
}

pub struct MutexBlockingInner {
    locked: bool,
    wait_queue: VecDeque<Arc<TaskControlBlock>>,
    id: usize,
}

impl MutexBlocking {
    pub fn new(id: usize) -> Self {
        let process = current_process();
        let mut process_inner = process.inner_exclusive_access();
        let tasks = &mut process_inner.tasks;
        for id in 0..tasks.len() {
            // &mut tasks[id].unwrap().inner_exclusive_access().mtx_allocation;
            let current_task = tasks[id].as_ref();
            if let Some(current_task) = current_task {
                let allocation = &mut current_task.inner_exclusive_access().mtx_allocation;
                while allocation.len() < id + 1 { // expand allocation
                    allocation.push(0);
                }
                let need = &mut current_task.inner_exclusive_access().mtx_need;
                while need.len() < id + 1 { // expand need
                    need.push(0);
                }
            }
        }
        Self {
            inner: unsafe {
                UPSafeCell::new(MutexBlockingInner {
                    locked: false,
                    wait_queue: VecDeque::new(),
                    id: id,
                })
            },
        }
    }
}

impl Mutex for MutexBlocking {
    fn islocked(&self) -> usize {
        if self.inner.exclusive_access().locked {
            return 0;
        }
        else {
            return 1;
        }
    }
    
    fn lock(&self) {
        let mut mutex_inner = self.inner.exclusive_access();
        if mutex_inner.locked {
            mutex_inner.wait_queue.push_back(current_task().unwrap());
            drop(mutex_inner);
            let cur_task = current_task().unwrap();
            let need = &mut cur_task.inner_exclusive_access().mtx_need;
            let mtx_id = self.inner.exclusive_access().id;
            need[mtx_id] = 1; // need the lock
            block_current_and_run_next();
        } else {
            let cur_task = current_task().unwrap();
            let need = &mut cur_task.inner_exclusive_access().mtx_need;
            let mtx_id = self.inner.exclusive_access().id;
            need[mtx_id] = 0; // no longer need the lock 
            let allocation = &mut cur_task.inner_exclusive_access().mtx_allocation;
            allocation[mtx_id] = 1; // alloc current lock to the thread
            mutex_inner.locked = true;
        }
    }

    fn unlock(&self) {
        let mut mutex_inner = self.inner.exclusive_access();
        assert!(mutex_inner.locked);
        if let Some(waking_task) = mutex_inner.wait_queue.pop_front() {
            // 所有权转移给waiting task
            {
                let cur_task = current_task().unwrap();
                let mtx_id = self.inner.exclusive_access().id;
                let allocation = &mut cur_task.inner_exclusive_access().mtx_allocation;
                allocation[mtx_id] = 0; // clear allocation
                let next_allocation = &mut waking_task.inner_exclusive_access().mtx_allocation;
                next_allocation[mtx_id] = 1;
                let next_need = &mut waking_task.inner_exclusive_access().mtx_need;
                next_need[mtx_id] = 0; // no longer waiting
            }

            add_task(waking_task);
        } else {
            mutex_inner.locked = false;
        }
    }
}
