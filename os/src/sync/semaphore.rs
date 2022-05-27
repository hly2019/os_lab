use crate::sync::UPSafeCell;
use crate::task::{add_task, block_current_and_run_next, current_task, TaskControlBlock, current_process};
use alloc::{collections::VecDeque, sync::Arc};

pub struct Semaphore {
    pub inner: UPSafeCell<SemaphoreInner>,
}

pub struct SemaphoreInner {
    pub count: isize,
    pub wait_queue: VecDeque<Arc<TaskControlBlock>>,
    pub id: usize,
}

impl Semaphore {
    pub fn new(res_count: usize, id: usize) -> Self {
        let process = current_process();
        let mut process_inner = process.inner_exclusive_access();
        let tasks = &mut process_inner.tasks;
        for id in 0..tasks.len() {
            // &mut tasks[id].unwrap().inner_exclusive_access().mtx_allocation;
            let current_task = tasks[id].as_ref();
            if let Some(current_task) = current_task {
                let allocation = &mut current_task.inner_exclusive_access().sem_allocation;
                while allocation.len() < id + 1 { // expand allocation
                    allocation.push(0);
                }
                let need = &mut current_task.inner_exclusive_access().sem_need;
                while need.len() < id + 1 { // expand need
                    need.push(0);
                }
            }
        }
        Self {
            inner: unsafe {
                UPSafeCell::new(SemaphoreInner {
                    count: res_count as isize,
                    wait_queue: VecDeque::new(),
                    id: id,
                })
            },
        }
    }

    pub fn up(&self) { //
        let mut inner = self.inner.exclusive_access();
        inner.count += 1;
        inner.wait_queue.push_back(current_task().unwrap());
        let cur_task = current_task().unwrap();
        let need = &mut cur_task.inner_exclusive_access().sem_need;
        let sem_id = self.inner.exclusive_access().id;
        need[sem_id] -= 1; // not need the lock
        if inner.count <= 0 {
            if let Some(task) = inner.wait_queue.pop_front() {
                inner.wait_queue.push_back(current_task().unwrap());
                {
                    let allocation = &mut task.inner_exclusive_access().sem_allocation;
                    let need = &mut task.inner_exclusive_access().sem_need;
                    let sem_id = self.inner.exclusive_access().id;
                    allocation[sem_id] += 1; // alloc the lock
                    need[sem_id] -= 1; // not need the lock
                }
                add_task(task);
            }
        }
    }

    pub fn down(&self) { // 分配资源
        let mut inner = self.inner.exclusive_access();
        inner.count -= 1;
        if inner.count < 0 {
            inner.wait_queue.push_back(current_task().unwrap());
            let cur_task = current_task().unwrap();
            let need = &mut cur_task.inner_exclusive_access().sem_need;
            let sem_id = self.inner.exclusive_access().id;
            need[sem_id] += 1; // need the lock
            drop(inner);
            block_current_and_run_next();
        }
        else {
            inner.wait_queue.push_back(current_task().unwrap());
            let cur_task = current_task().unwrap();
            let allocation = &mut cur_task.inner_exclusive_access().sem_allocation;
            let sem_id = self.inner.exclusive_access().id;
            allocation[sem_id] += 1; // alloc the lock
        }
    }
}
