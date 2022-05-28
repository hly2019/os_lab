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
    pub fn update(&self) {
        let inner = self.inner.exclusive_access();
        let sem_id = inner.id;
        let mut count = inner.count;
        count -= 1;
        if count < 0 {
            let cur_task = current_task().unwrap();
            let need = &mut cur_task.inner_exclusive_access().sem_need;
            // println!("before update, need is: {}", need[sem_id]);
            // println!("_____________________________________________________");
            need[sem_id] += 1; // need the lock
            // println!("after update, need is: {}", need[sem_id]);
            // println!("_____________________________________________________");
        }
        else {
            let cur_task = current_task().unwrap();
            let allocation = &mut cur_task.inner_exclusive_access().sem_allocation;
            // let sem_id = self.inner.exclusive_access().id;
            allocation[sem_id] += 1;
        }
    }
    pub fn new(res_count: usize, id: usize) -> Self {
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
        // inner.wait_queue.push_back(current_task().unwrap());
        // let cur_task = current_task().unwrap();
        // let need = &mut cur_task.inner_exclusive_access().sem_need;
        // println!("for this thread, before up, need is {}", need[sem_id]);
        // // need[sem_id] -= 1; // not need the lock
        // println!("for this thread, after up, need is {}", need[sem_id]);
        
        let sem_id = inner.id;
        if inner.count <= 0 {
            if let Some(task) = inner.wait_queue.pop_front() {
                // inner.wait_queue.push_back(current_task().unwrap());
                {
                    {
                        let next_allocation = &mut task.inner_exclusive_access().sem_allocation;
                        // let sem_id = self.inner.exclusive_access().id;
                        next_allocation[sem_id] += 1; // alloc the lock
                    }
                    {
                        let next_need = &mut task.inner_exclusive_access().sem_need;
                        // println!("_____________________________________________________");
                        // println!("before up, need is: {}", next_need[sem_id]);
                        next_need[sem_id] -= 1; // not need the lock
                        // println!("after up, need is: {}", next_need[sem_id]);
                        // println!("_____________________________________________________");


                    }
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
            drop(inner);
            block_current_and_run_next();
        }
    }
}
