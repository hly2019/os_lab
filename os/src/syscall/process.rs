//! Process management syscalls

use crate::config::MAX_SYSCALL_NUM;
use crate::task::{exit_current_and_run_next, suspend_current_and_run_next, current_user_token, TaskStatus};
use crate::timer::get_time_us;
use crate::mm::*;
#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

#[derive(Clone, Copy)]
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

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}

// YOUR JOB: 引入虚地址后重写 sys_get_time
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    let _us = get_time_us();
    let token = current_user_token();
    let page_table = PageTable::from_token(token);
    let start_va = VirtAddr::from(_ts as usize);
    let vpn = start_va.floor();
    let ppn = page_table.translate(vpn).unwrap().ppn().0;
    let time_ptr = ppn << 12 | start_va.page_offset(); // ppn左移12位拼上offset
    unsafe {
        *(time_ptr as *mut TimeVal) = TimeVal {
            sec: _us / 1_000_000,
            usec: _us % 1_000_000,
        };
    }
    0
}

// CLUE: 从 ch4 开始不再对调度算法进行测试~
pub fn sys_set_priority(_prio: isize) -> isize {
    -1
}

// YOUR JOB: 扩展内核以实现 sys_mmap 和 sys_munmap
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    -1
}

pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    -1
}

// YOUR JOB: 引入虚地址后重写 sys_task_info
pub fn sys_task_info(ti: *mut TaskInfo) -> isize {
    let token = current_user_token();
    let page_table = PageTable::from_token(token);
    let start_va = VirtAddr::from(ti as usize);
    let vpn = start_va.floor();
    let ppn = page_table.translate(vpn).unwrap().ppn().0;
    let time_ptr = ppn << 12 | start_va.page_offset(); // ppn左移12位拼上offset

    -1
}
