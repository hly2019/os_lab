use crate::config::{ MAX_SYSCALL_NUM};
use crate::task::{exit_current_and_run_next, suspend_current_and_run_next, TaskStatus, get_cur_task_systimes,  get_cur_task_first_invoked_time};
use crate::timer::{get_time_us, get_time};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

pub struct TaskInfo {
    pub status: TaskStatus,
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
    pub time: usize,
}

pub fn sys_exit(exit_code: i32) -> ! {
    info!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}

pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    let us = get_time_us();
    unsafe {
        *ts = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}

// YOUR JOB: Finish sys_task_info to pass testcases
pub fn sys_task_info(ti: *mut TaskInfo) -> isize {
    // let status =  get_cur_task_status();
    let arr =  get_cur_task_systimes();
    unsafe {
        for i in 0..MAX_SYSCALL_NUM {
            (*ti).syscall_times[i] = arr[i];
        }
        (*ti).status = TaskStatus::Running;
        (*ti).time = (get_time() - get_cur_task_first_invoked_time()) / 10000;
    }
    
    0
}
