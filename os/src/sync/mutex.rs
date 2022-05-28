use super::UPSafeCell;
use crate::task::{TaskControlBlock};
use crate::task::{add_task, current_task};
use crate::task::{block_current_and_run_next, suspend_current_and_run_next};
use alloc::{collections::VecDeque, sync::Arc};

pub trait Mutex: Sync + Send {
    fn lock(&self);
    fn unlock(&self);
    fn islocked(&self) -> usize;
    fn get_id(&self) -> usize;
    fn update(&self);
}

pub struct MutexSpin {
    // locked: UPSafeCell<bool>,
    inner: UPSafeCell<MutexSpinInner>,
    id: usize,


}
pub struct MutexSpinInner {
    locked: bool,
    // wait_queue: VecDeque<Arc<TaskControlBlock>>,
}
impl MutexSpin {
    pub fn new(id: usize) -> Self {
        println!("in mutex spin new");
        Self {
            inner: unsafe {
                UPSafeCell::new(MutexSpinInner {
                    locked: false,
                })
            },
            id: id,
        }
    }
}

impl Mutex for MutexSpin {
    fn get_id(&self) -> usize {
        return self.id;
    }
    fn update(&self) {
        let mutex_inner = self.inner.exclusive_access();
        if mutex_inner.locked {
            drop(mutex_inner);
            let cur_task = current_task().unwrap();
            let need = &mut cur_task.inner_exclusive_access().mtx_need;
            let mtx_id = self.id;
            need[mtx_id] = 1; // need the lock
        } else {
            let cur_task = current_task().unwrap();
            let mtx_id = self.id;
            // {
            //     let need = &mut cur_task.inner_exclusive_access().mtx_need;
            //     need[mtx_id] = 0; // no longer need the lock 
            // }
            {
                let allocation = &mut cur_task.inner_exclusive_access().mtx_allocation;
                allocation[mtx_id] = 1; // alloc current lock to the thread
            }
        }
    }
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
            if mutex_inner.locked {
                drop(mutex_inner);
                suspend_current_and_run_next();
                continue;
            } else {
                let mtx_id = self.id;
                let cur_task = current_task().unwrap();
                {            
                    let need = &mut cur_task.inner_exclusive_access().mtx_need;
                    need[mtx_id] = 0; // no longer need the lock 
                }
                {
                    let allocation = &mut cur_task.inner_exclusive_access().mtx_allocation;
                    allocation[mtx_id] = 1; // alloc current lock to the thread
                }
                mutex_inner.locked = true;
                return;
            }
        }
    }

    fn unlock(&self) {
        let mut mutex_inner = self.inner.exclusive_access();
        let mtx_id = self.id;
        let cur_task = current_task().unwrap();
        {
            let allocation = &mut cur_task.inner_exclusive_access().mtx_allocation;
            allocation[mtx_id] = 0; // alloc current lock to the thread
        }
        // let mut locked = self.locked.exclusive_access();
        mutex_inner.locked = false;
    }
}

pub struct MutexBlocking {
    inner: UPSafeCell<MutexBlockingInner>,
    id: usize,

}

pub struct MutexBlockingInner {
    locked: bool,
    wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl MutexBlocking {
    pub fn new(id: usize) -> Self {
        println!("in mutex blocking new");
        Self {
            inner: unsafe {
                UPSafeCell::new(MutexBlockingInner {
                    locked: false,
                    wait_queue: VecDeque::new(),
                })
            },
            id: id,
        }
    }
}

impl Mutex for MutexBlocking {
    fn get_id(&self) -> usize {
        return self.id;
    }
    fn update(&self) {
        println!("in update");
        let mutex_inner = self.inner.exclusive_access();
        if mutex_inner.locked {
            // mutex_inner.wait_queue.push_back(current_task().unwrap());
            drop(mutex_inner);
            let cur_task = current_task().unwrap();
            let need = &mut cur_task.inner_exclusive_access().mtx_need;
            let mtx_id = self.id;
            need[mtx_id] = 1; // need the lock
        } else {
            let cur_task = current_task().unwrap();
            let mtx_id = self.get_id();
            {
                let need = &mut cur_task.inner_exclusive_access().mtx_need;
                need[mtx_id] = 0; // no longer need the lock 
            }
            {
                let allocation = &mut cur_task.inner_exclusive_access().mtx_allocation;
                allocation[mtx_id] = 1; // alloc current lock to the thread
            }
        }
    }
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
            block_current_and_run_next();
        } else {
                mutex_inner.locked = true;
        }
    }

    fn unlock(&self) {
        let mut mutex_inner = self.inner.exclusive_access();
        assert!(mutex_inner.locked);
        if let Some(waking_task) = mutex_inner.wait_queue.pop_front() {
            // println!("waking task: {}", )
            // 所有权转移给waiting task
            {
                let mtx_id = self.id;
                {
                    let cur_task = current_task().unwrap();
                    let allocation = &mut cur_task.inner_exclusive_access().mtx_allocation;
                    allocation[mtx_id] = 0; // clear allocation
                }
                {
                    println!("waking ?");
                    let next_allocation = &mut waking_task.inner_exclusive_access().mtx_allocation;
                    next_allocation[mtx_id] = 1;
                }   
                {
                    println!("waking 2");
                    let next_need = &mut waking_task.inner_exclusive_access().mtx_need;
                    next_need[mtx_id] = 0; // no longer waiting
                }
            }
            add_task(waking_task);
        } else {
            println!("direct unlock");
            mutex_inner.locked = false;
        }
    }
}
