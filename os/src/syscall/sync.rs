use crate::console::print;
use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
use alloc::vec::Vec;

pub fn sys_sleep(ms: usize) -> isize {
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}

// LAB5 HINT: you might need to maintain data structures used for deadlock detection
// during sys_mutex_* and sys_semaphore_* syscalls
pub fn sys_mutex_create(blocking: bool) -> isize {
    let process = current_process();

    let mut process_inner = process.inner_exclusive_access();
    let maxid = process_inner.mutex_list.len();

    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        println!("test before");
        let tasks = &mut process_inner.tasks;
        println!("flag2 is here");
        for i in 0..tasks.len() {
            // &mut tasks[id].unwrap().inner_exclusive_access().mtx_allocation;
            let current_task = tasks[i].as_ref();
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
        let mutex: Option<Arc<dyn Mutex>> = if !blocking {
            Some(Arc::new(MutexSpin::new(id)))
        } else {
            Some(Arc::new(MutexBlocking::new(id)))
        };
        process_inner.mutex_list[id] = mutex;
        println!("test");
        id as isize
    } else {
        println!("test before 2 ");
        let tasks = &mut process_inner.tasks;
        println!("test0!!!!!");
        for i in 0..tasks.len() {
            // &mut tasks[id].unwrap().inner_exclusive_access().mtx_allocation;
            let current_task = tasks[i].as_ref();
            if let Some(current_task) = current_task {
                println!("test1!!!!!");
                {
                    let allocation = &mut current_task.inner_exclusive_access().mtx_allocation;
                    println!("testxxx!!!!!");
    
                    while allocation.len() < maxid + 1 { // expand allocation
                        allocation.push(0);
                    }
                }
                {
                    let need = &mut current_task.inner_exclusive_access().mtx_need;
                    println!("testyyy!!!!!");
                    while need.len() < maxid + 1 { // expand need
                        need.push(0);
                    }
                }

            }
        }
        
        let mutex: Option<Arc<dyn Mutex>> = if !blocking {
            Some(Arc::new(MutexSpin::new(process_inner.mutex_list.len())))
        } else {
            Some(Arc::new(MutexBlocking::new(process_inner.mutex_list.len())))
        };
        process_inner.mutex_list.push(mutex);
        println!("test2!!!!!");
        process_inner.mutex_list.len() as isize - 1
    }
}

// LAB5 HINT: Return -0xDEAD if deadlock is detected
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    println!("in kernel, in locking");

    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let enable = process_inner.enable_detect;
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    println!("in kernel, before detecting");
    // mutex;
    if detect_deadlock(1, enable) == false {
        return -0xDEAD;
    }
    println!("in kernel, before locking");
    mutex.lock();
    0
}

pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    mutex.unlock();
    0
}

pub fn sys_semaphore_create(res_count: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count, id)));
        id
    } else {
        let sid = process_inner.semaphore_list.len();
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count, sid))));
        process_inner.semaphore_list.len() - 1
    };
    id as isize
}

pub fn sys_semaphore_up(sem_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    drop(process_inner);
    sem.up();
    0
}

// LAB5 HINT: Return -0xDEAD if deadlock is detected
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    if detect_deadlock(0, process_inner.enable_detect) == false {
        return -0xDEAD;
    }
    drop(process_inner);
    sem.down();
    0
}

pub fn sys_condvar_create(_arg: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}

pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}

pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}
pub fn detect_deadlock(is_mtx: usize, _enabled: usize) -> bool { // return true: no deadlock
    if _enabled == 0 {
        return true; // ok
    }
    else if _enabled != 1 {
        return false;
    }
    print!("is mutex?: {}",is_mtx);
    if is_mtx == 1 { // mtx
        println!("----------------------");
        println!("in detetcting mutex");
        let mut work: Vec<usize> = Vec::new();
        let mut finish: Vec<bool> = Vec::new();
        let process = current_process();
        let mut process_inner = process.inner_exclusive_access();
        println!("flag1!!!!");
        let mut task_len = 0;
        {
            task_len = process_inner.tasks.len();
        }
        // let tasks = &mut process_inner.tasks;
        let mtx_list = &mut process_inner.mutex_list;
        println!("flag2!!!!, mtx lis len is : {}", mtx_list.len());
        for i in 0..mtx_list.len() {
            // println!("mtx is: {}",mtx_list[i].unwrap().islocked() );
            if let Some(mtx) = &mut mtx_list[i] {
                if mtx.islocked() == 1 { // 资源可用
                    work.push(1);
                }
                else {
                    work.push(0);
                }
            }
            else {
                work.push(0);
            }
        }
        for i in 0..task_len {
            finish.push(false);
        }

        while true {
            let mut found = false;
            let tasks = &mut process_inner.tasks;
            println!("flag3!!!!");
            for i in 0..task_len {
                println!("work len is: {}, hahahaa", work.len());

                if finish[i] != false {
                    continue;
                }
                if let Some(task) = &mut tasks[i] {
                    println!("flag in some");
                    println!("work len: {}", work.len());
                    let mut err = false;
                    {
                        let need = &mut task.inner_exclusive_access().mtx_need;
                        for j in 0..work.len() {
                            println!("need: {}, work: {}", need[j], work[j]);
                            if need[j] > work[j] {
                                err = true;
                                break;
                            }
                        }
                    }
                    if err == false {
                        found = true;
                        let allocation = &mut task.inner_exclusive_access().mtx_allocation;
                        for j in 0..work.len() {
                            work[j] += allocation[j];
                        }
                        finish[i] = true;
                    }
                }
            }
            if found == false {
                break;
            }
        }
        println!("flag4!!!!");
        print!("task_len is: {}", task_len);
        for i in 0..task_len {
            if finish[i] == false {
                print!("-------------------------");
                return false;
            }
        }
        print!("MMMMMMMMMMMMMMMMMMMMMMM");
        return true;
    }
    else { // sem
        let process = current_process();
        let mut process_inner = process.inner_exclusive_access();
        let mut task_len = 0;
        {
            task_len = process_inner.tasks.len();
        }
        // let tasks = &mut process_inner.tasks;
        let sem_list = &mut process_inner.semaphore_list;
        let mut work: Vec<usize> = Vec::new();
        let mut finish: Vec<bool> = Vec::new();
        for i in 0..sem_list.len() {
            if let Some(sem) = &mut sem_list[i] {
                let count = sem.inner.exclusive_access().count;
                if count > 0 { // 资源可用
                    work.push(count as usize);
                }
            }
            else {
                work.push(0);
            }
        }
        for i in 0..task_len {
            finish.push(false);
        }

        while true {
            let mut found = false;
            let tasks = &mut process_inner.tasks;
            for i in 0..task_len {
                if finish[i] != false {
                    continue;
                }
                if let Some(task) = &mut tasks[i] {
                    let need = &mut task.inner_exclusive_access().sem_need;
                    let need_len = need.len();
                    let mut err = false;
                    for j in 0..need.len() {
                        if need[j] > work[j] {
                            err = true;
                            break;
                        }
                    }
                    if err == false {
                        found = true;
                        let allocation = &mut task.inner_exclusive_access().sem_allocation;
                        for j in 0..need_len {
                            work[j] += allocation[j];
                        }
                        finish[i] = true;
                    }
                }
            }
            if found == false {
                break;
            }
        }
        for i in 0..task_len {
            if finish[i] == false {
                return false;
            }
        }
        return true;
    }
}
// LAB5 YOUR JOB: Implement deadlock detection, but might not all in this syscall
pub fn sys_enable_deadlock_detect(_enabled: usize) -> isize {
    if _enabled != 0 && _enabled != 1 {
        return -1;
    }
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    process_inner.enable_detect = _enabled;
    1
}
